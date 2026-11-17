---
title: "genui: ast/type-aliases/Expr"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [ast](/docs/dev/typescript/genui/ast/) / Expr

# Type Alias: Expr

> **Expr** = \{ `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"literal"`; `value`: [`Scalar`](/docs/dev/typescript/genui/types/type-aliases/scalar/); \} \| \{ `parts`: readonly `Expr`[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `quasis`: readonly `string`[]; `type`: `"template"`; \} \| \{ `name`: `string`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"state"`; \} \| \{ `name`: `string`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"ident"`; \} \| \{ `items`: readonly `Expr`[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"list"`; \} \| \{ `entries`: readonly `object`[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"object"`; \} \| \{ `op`: `"!"` \| `"-"`; `operand`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"unary"`; \} \| \{ `left`: `Expr`; `op`: [`BinaryOperator`](/docs/dev/typescript/genui/ast/type-aliases/binaryoperator/); `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `right`: `Expr`; `type`: `"binary"`; \} \| \{ `else`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `test`: `Expr`; `then`: `Expr`; `type`: `"conditional"`; \} \| \{ `name`: `string`; `object`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"member"`; \} \| \{ `index`: `Expr`; `object`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"index"`; \} \| \{ `args`: readonly `Expr`[]; `name`: `string`; `named`: readonly [`NamedArg`](/docs/dev/typescript/genui/ast/interfaces/namedarg/)[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"call"`; \} \| \{ `args`: readonly `Expr`[]; `name`: `string`; `named`: readonly [`NamedArg`](/docs/dev/typescript/genui/ast/interfaces/namedarg/)[]; `object`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"method"`; \} \| \{ `body`: `Expr`; `params`: readonly `string`[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"arrow"`; \}

Expression syntax; arrows describe only named functions or list callbacks, never host code.

## Union Members

### Type Literal

\{ `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"literal"`; `value`: [`Scalar`](/docs/dev/typescript/genui/types/type-aliases/scalar/); \}

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### type

> `readonly` **type**: `"literal"`

Syntax or interpreter-value discriminator; determines the other fields.

#### value

> `readonly` **value**: [`Scalar`](/docs/dev/typescript/genui/types/type-aliases/scalar/)

Literal value or unevaluated expression belonging to this syntax item.

***

### Type Literal

\{ `parts`: readonly `Expr`[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `quasis`: readonly `string`[]; `type`: `"template"`; \}

`quasis.length === parts.length + 1`.

#### parts

> `readonly` **parts**: readonly `Expr`[]

Interpolated template expressions in source order.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### quasis

> `readonly` **quasis**: readonly `string`[]

Literal template segments; contains one more segment than parts.

#### type

> `readonly` **type**: `"template"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `name`: `string`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"state"`; \}

`$name`; `name` excludes the `$`.

#### name

> `readonly` **name**: `string`

Identifier, property, method or callee name; state names exclude the dollar prefix.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### type

> `readonly` **type**: `"state"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `name`: `string`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"ident"`; \}

A named value or an arrow parameter.

#### name

> `readonly` **name**: `string`

Identifier, property, method or callee name; state names exclude the dollar prefix.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### type

> `readonly` **type**: `"ident"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `items`: readonly `Expr`[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"list"`; \}

#### items

> `readonly` **items**: readonly `Expr`[]

Ordered list-element expressions.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### type

> `readonly` **type**: `"list"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `entries`: readonly `object`[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"object"`; \}

#### entries

> `readonly` **entries**: readonly `object`[]

Object entries in source insertion order.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

Source location of this syntax item.

#### type

> `readonly` **type**: `"object"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `op`: `"!"` \| `"-"`; `operand`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"unary"`; \}

#### op

> `readonly` **op**: `"!"` \| `"-"`

Whitelisted pure operator spelling.

#### operand

> `readonly` **operand**: `Expr`

Unevaluated prefix operand.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### type

> `readonly` **type**: `"unary"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `left`: `Expr`; `op`: [`BinaryOperator`](/docs/dev/typescript/genui/ast/type-aliases/binaryoperator/); `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `right`: `Expr`; `type`: `"binary"`; \}

#### left

> `readonly` **left**: `Expr`

Left operand; evaluated before the right operand.

#### op

> `readonly` **op**: [`BinaryOperator`](/docs/dev/typescript/genui/ast/type-aliases/binaryoperator/)

Whitelisted pure operator spelling.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### right

> `readonly` **right**: `Expr`

Right operand; logical operators evaluate this lazily.

#### type

> `readonly` **type**: `"binary"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `else`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `test`: `Expr`; `then`: `Expr`; `type`: `"conditional"`; \}

#### else

> `readonly` **else**: `Expr`

Branch evaluated for a false test.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### test

> `readonly` **test**: `Expr`

Conditional test expression.

#### then

> `readonly` **then**: `Expr`

Branch evaluated for a truthy test.

#### type

> `readonly` **type**: `"conditional"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `name`: `string`; `object`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"member"`; \}

`object.name`; `pos` is the property name.

#### name

> `readonly` **name**: `string`

Identifier, property, method or callee name; state names exclude the dollar prefix.

#### object

> `readonly` **object**: `Expr`

Receiver expression for a property, index or method access.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### type

> `readonly` **type**: `"member"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `index`: `Expr`; `object`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"index"`; \}

#### index

> `readonly` **index**: `Expr`

Index expression; lists and strings use numbers, objects use text.

#### object

> `readonly` **object**: `Expr`

Receiver expression for a property, index or method access.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### type

> `readonly` **type**: `"index"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `args`: readonly `Expr`[]; `name`: `string`; `named`: readonly [`NamedArg`](/docs/dev/typescript/genui/ast/interfaces/namedarg/)[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"call"`; \}

`name(...)`: a component, action or function; `pos` is the name (the node key).

#### args

> `readonly` **args**: readonly `Expr`[]

Positional call arguments in source order.

#### name

> `readonly` **name**: `string`

Identifier without a state prefix.

#### named

> `readonly` **named**: readonly [`NamedArg`](/docs/dev/typescript/genui/ast/interfaces/namedarg/)[]

Top-level definitions in source insertion order.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

Source location of this syntax item.

#### type

> `readonly` **type**: `"call"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `args`: readonly `Expr`[]; `name`: `string`; `named`: readonly [`NamedArg`](/docs/dev/typescript/genui/ast/interfaces/namedarg/)[]; `object`: `Expr`; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"method"`; \}

`object.name(...)`; `pos` is the method name.

#### args

> `readonly` **args**: readonly `Expr`[]

Positional call arguments in source order.

#### name

> `readonly` **name**: `string`

Identifier without a state prefix.

#### named

> `readonly` **named**: readonly [`NamedArg`](/docs/dev/typescript/genui/ast/interfaces/namedarg/)[]

Top-level definitions in source insertion order.

#### object

> `readonly` **object**: `Expr`

Receiver expression for a property, index or method access.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

Source location of this syntax item.

#### type

> `readonly` **type**: `"method"`

Syntax or interpreter-value discriminator; determines the other fields.

***

### Type Literal

\{ `body`: `Expr`; `params`: readonly `string`[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"arrow"`; \}

#### body

> `readonly` **body**: `Expr`

Pure function body; never platform or JavaScript code.

#### params

> `readonly` **params**: readonly `string`[]

Lexical callback parameter names.

#### pos

> `readonly` **pos**: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/)

One-based UTF-16 source position used for diagnostics and component identity.

#### type

> `readonly` **type**: `"arrow"`

Syntax or interpreter-value discriminator; determines the other fields.
