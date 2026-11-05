; Test suites. GoogleTest-style macros (TEST, TEST_F, TEST_P, TYPED_TEST,
; TYPED_TEST_P) and Catch2-style macros (TEST_CASE, SCENARIO), plus the
; Unity/CUnit convention of naming test functions test_*.

; When the grammar folds the invocation into a function definition (the
; C++ shape of GoogleTest tests, and plain test_* functions everywhere),
; the whole definition is test code, the way a whole #[cfg(test)] module
; is in Rust.
(
  (function_definition
    declarator: (function_declarator declarator: (identifier) @test_name))
  (#match? @test_name "^test_|^(TEST|TEST_F|TEST_P|TYPED_TEST|TYPED_TEST_P|TEST_CASE|SCENARIO)$")
) @test

; Where the grammar splits the invocation from its body (the C shape: the
; call becomes a top-level expression statement and the body a sibling
; block), both halves count as test.
(
  (translation_unit
    (expression_statement
      (call_expression
        function: (identifier) @test_macro)) @test
    .
    (compound_statement) @test)
  (#match? @test_macro "^(TEST|TEST_F|TEST_P|TYPED_TEST|TYPED_TEST_P|TEST_CASE|SCENARIO)$")
)

; A test call nested inside other code: the call itself is test code.
(
  (call_expression
    function: (identifier) @test_macro)
  (#match? @test_macro "^(TEST|TEST_F|TEST_P|TYPED_TEST|TYPED_TEST_P|TEST_CASE|SCENARIO)$")
) @test
