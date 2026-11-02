; Jest-style suites and tests: describe, it, and test calls, including the
; x/f-prefixed (skipped/focused) forms. The whole call is test code, the
; way a whole #[cfg(test)] module is in Rust.
(
  (call_expression
    function: (identifier) @fn_name)
  (#match? @fn_name "^([xf])?(describe|it|test)(\\.[a-zA-Z]+)?$")
) @test

; Member modifiers: describe.only, it.each, test.skip, ...
(
  (call_expression
    function: (member_expression
      object: (identifier) @obj_name))
  (#match? @obj_name "^([xf])?(describe|it|test)$")
) @test
