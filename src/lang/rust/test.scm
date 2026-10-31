; `#[cfg(test)]` module: the whole module is test code.
(
  (attribute_item) @attr
  .
  (mod_item) @test
  (#match? @attr "^#\\[\\s*cfg\\s*\\(\\s*test\\s*\\)\\s*\\]$")
)

; `#[test]` function: the whole function is test code.
(
  (attribute_item) @attr
  .
  (function_item) @test
  (#match? @attr "^#\\[\\s*test\\s*\\]$")
)
