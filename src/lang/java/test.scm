; JUnit methods: @Test, @ParameterizedTest, @RepeatedTest, @TestFactory,
; @TestTemplate (plain and argument-carrying forms, TestNG's @Test too).
; The whole declaration — annotations included — is test code, the way a
; whole #[cfg(test)] module is in Rust.
(
  (method_declaration
    (modifiers
      (marker_annotation
        name: (identifier) @anno)))
  (#match? @anno "^(Test|ParameterizedTest|RepeatedTest|TestFactory|TestTemplate)$")
) @test

(
  (method_declaration
    (modifiers
      (annotation
        name: (identifier) @anno)))
  (#match? @anno "^(Test|ParameterizedTest|RepeatedTest|TestFactory|TestTemplate)$")
) @test

; @Nested test classes: the whole class, helpers included, is test code.
(
  (class_declaration
    (modifiers
      (marker_annotation
        name: (identifier) @anno)))
  (#match? @anno "^(Nested)$")
) @test
