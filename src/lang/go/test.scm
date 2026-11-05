; testing-package functions: TestXxx, BenchmarkXxx, FuzzXxx, ExampleXxx,
; including their subtests, which nest inside the function body. `go test`
; only runs them from *_test.go files, which the test globs already cover;
; this catches the strays elsewhere. The first letter after the prefix
; must not be lowercase (the `go vet` rule), so Testfoo and Testing skip.
(
  (function_declaration
    name: (identifier) @fn_name)
  (#match? @fn_name "^(Test|Benchmark|Fuzz|Example)[^a-z]")
) @test
