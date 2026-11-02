; pytest-style test functions. Captured at the decorated_definition when
; one is present, so decorators count as test, and at the bare function
; otherwise. The two ranges overlap; the classifier handles that.
(
  (decorated_definition
    definition: (function_definition name: (identifier) @test_name))
  (#match? @test_name "^test_")
) @test

(
  (function_definition name: (identifier) @test_name)
  (#match? @test_name "^test_")
) @test

; unittest classes deriving TestCase, directly or qualified
; (class X(TestCase), class X(unittest.TestCase)).
(
  (class_definition
    superclasses: (argument_list (identifier) @base))
  (#eq? @base "TestCase")
) @test

(
  (class_definition
    superclasses: (argument_list (attribute attribute: (identifier) @base)))
  (#eq? @base "TestCase")
) @test
