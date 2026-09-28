package functions

import (
	"context"
	"errors"
	"fmt"
	"iter"
	"log/slog"
)

func (f *Function) DefaultModelID() string {
	if mr, ok := f.executor.(ModelRequired); ok {
		return mr.DefaultModelID()
	}
	return ""
}

func (f *Function) Executor() FunctionExecutor {
	return f.executor
}

func NewFunction(name, description string, input map[string]FunctionParameter, output map[string]FunctionParameter, executor FunctionExecutor) *Function {
	return &Function{
		Input:       input,
		Output:      output,
		Name:        name,
		Description: description,
		executor:    executor,
	}
}

// vhco:usecase dispatch.run_function(input: Data) -> iter.Seq2[Data, error] needs FunctionExecutor
// vhco:label Run a declarative function
// vhco:about Filters+validates the input against the schema, runs the attached executor, then filters/validates/transforms each streamed output chunk.
// vhco:step filter-input f.filterInput @function -- keep only declared input fields and inject declared defaults
// vhco:step validate-input f.validateInput @function -- reject missing-required and unexpected fields
// vhco:step resolve-converter resolveTextConverter @function -- pick a text transform (e.g. s2tw) from the input once
// vhco:step execute f.executor.Execute @executor -- run the executor and iterate its streamed Data chunks
// vhco:step filter-output f.filterOutput @function -- keep only declared output fields per chunk
// vhco:step apply-transforms f.applyTransforms @function -- run any per-field output Transform
// vhco:step convert-strings convertStrings @function -- apply the resolved text converter to string values
// vhco:error missing-executor -- the function has no attached executor => "missing executor" returns
// vhco:error invalid-input -- a required field is missing or an unexpected field is present => error returns
func (f *Function) Execute(ctx *ExecutionContext) iter.Seq2[Data, error] {
	return func(yield func(Data, error) bool) {
		if f.executor == nil {
			yield(nil, errors.New("missing executor"))
			return
		}
		ctx.Input = f.filterInput(ctx.Input)
		if err := f.validateInput(ctx.Input); err != nil {
			yield(nil, err)
			return
		}

		// The resolved input is a DEBUG diagnostic, never stdout. Printing it here put
		// every function's rendered input — prompts, user messages, RAG context — on
		// the stdout of the server AND of every CLI command, which breaks the contract
		// PROP-2026-0050 hardened: a one-off command's stdout is its RESULT, so
		// `aimanager exec … | jq .` must parse. (PROP-2026-0049 §3.4.)
		if slog.Default().Enabled(context.Background(), slog.LevelDebug) {
			slog.Debug("function: resolved input",
				slog.String("name", f.Name),
				slog.Any("input", ctx.Input),
			)
		}

		// Resolve text converter once from input — nil means no conversion.
		convert := resolveTextConverter(f.TextTransforms, ctx.Input)

		for rawOut, err := range f.executor.Execute(ctx) {
			out := rawOut
			if err == nil {
				out = f.filterOutput(out)
				if len(out) == 0 {
					// PROP-2026-0096 C-02 / INC-2026-0040: this loop's `continue`
					// used to conflate "the executor produced nothing" with "the
					// executor produced data, but nothing survived the declared
					// `output:` schema filter" — the latter made a successful-but-
					// misconfigured tool call indistinguishable from one that was
					// never called (exit 0, zero bytes, no error).
					//
					// The proposal's own Risks table anticipated the fix below —
					// still yield the empty chunk — could break a caller relying on
					// the silent continue. It does: Function.Execute is also how
					// workflows.go dispatches EVERY internal step, and a workflow's
					// intermediate steps deliberately produce keys outside the
					// workflow's own declared Output schema (see
					// internal/executors/workflows/one_frame_test.go, which asserts
					// a non-streaming workflow yields EXACTLY ONE frame — extra
					// frames duplicate `aimanager exec | jq .`'s stdout).
					// TestWorkflowYieldsOneFrameWhenNotStreaming confirmed this: an
					// intermediate step's schema-filtered-empty chunk started
					// surfacing as a second frame.
					//
					// Per that documented rollback ("keep the load-time check (R3)
					// only"), this stays a silent `continue` — no extra frame — but
					// still WARNs whenever the executor DID produce something that
					// nothing survived to keep, so the diagnosability gain (a log
					// line to find, where before there was none) ships without the
					// workflow regression. R3's load-time check (tool.go) is what
					// actually closes INC-2026-0040 for its confirmed case — a
					// non-exempt tool with a declared `output:` schema and empty
					// `response.output` is now rejected before it can ever reach
					// this path.
					if len(rawOut) > 0 {
						unmapped := make([]string, 0, len(rawOut))
						for k := range rawOut {
							unmapped = append(unmapped, k)
						}
						slog.Warn("function: output schema declared but response.output mapped nothing",
							slog.String("name", f.Name),
							slog.Any("unmapped_keys", unmapped),
						)
					}
					continue
				}
				err = f.validateOutput(out)
			}
			if err == nil && out != nil {
				err = f.applyTransforms(out, ctx)
			}
			if err == nil && convert != nil {
				err = convertStrings(out, convert)
			}
			if !yield(out, err) {
				return
			}
		}
	}
}

func (f *Function) filterOutput(output Data) Data {

	if len(f.Output) == 0 {
		return output
	}
	filtered := make(Data, len(f.Output))
	for name := range f.Output {
		if val, ok := output[name]; ok {
			filtered[name] = val
		}
	}
	return filtered
}

func (f *Function) filterInput(input Data) Data {
	filtered := make(Data, len(f.Input))
	for name, param := range f.Input {
		if val, ok := input[name]; ok {
			filtered[name] = val
		} else if param.Default != nil {
			// Inject declared defaults so templates see them instead of
			// rendering "<no value>" for omitted-but-defaulted inputs.
			filtered[name] = param.Default
		}
	}
	return filtered
}

func (f *Function) validateInput(input Data) error {
	if input == nil {
		input = map[string]any{}
	}
	for name, param := range f.Input {
		_, exists := input[name]
		if param.Required && !exists {
			if param.RequiredBecause != "" {
				return fmt.Errorf("missing required field: %s — %s", name, param.RequiredBecause)
			}
			return fmt.Errorf("missing required field: %s", name)
		}
		if !exists {
			continue
		}
		// if err := checkType(name, param.Type, val); err != nil {
		// 	return err
		// }
	}
	for key := range input {
		if _, ok := f.Input[key]; !ok {
			return fmt.Errorf("unexpected input field: %s", key)
		}
	}
	return nil
}

func (f *Function) validateOutput(data Data) error {
	return nil
}

// applyTransforms runs any Transform declared on output FunctionParameters.
// Called on every yielded chunk; only processes keys present in both the
// output schema and the chunk data.
func (f *Function) applyTransforms(out Data, ctx *ExecutionContext) error {
	for key, param := range f.Output {
		if param.Transform == nil || param.Transform.Op == "" {
			continue
		}
		if _, exists := out[key]; !exists {
			continue
		}
		if err := applyTransform(param.Transform, key, out, ctx); err != nil {
			return fmt.Errorf("transform field %q: %w", key, err)
		}
	}
	return nil
}
