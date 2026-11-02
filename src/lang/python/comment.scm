; # line comments.
(comment) @comment

; Bare string statements: docstrings, and the bare-string idiom Python
; uses for block comments (it has no block comment syntax).
(expression_statement (string)) @comment
