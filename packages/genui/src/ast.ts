/** Serializable syntax records for the credited upstream expression language; no executable values cross the boundary.
 * @module
 */
import type { Pos } from "./diagnostics.js";
import type { Scalar } from "./types.js";
/** Allowed pure binary operators, in ascending precedence groups. */
export type BinaryOperator = "??" | "||" | "&&" | "==" | "!=" | "===" | "!==" | "<" | "<=" | ">" | ">=" | "+" | "-" | "*" | "/" | "%" | "**";
/** A named call argument with its own source position. */
export interface NamedArg {
    /** Identifier without a state prefix. */
    readonly name: string;
    /** Unevaluated expression. */
    readonly value: Expr;
    /** Source location of this syntax item. */
    readonly pos: Pos;
}
/** Expression syntax; arrows describe only named functions or list callbacks, never host code. */
export type Expr = {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "literal";
    /** Literal value or unevaluated expression belonging to this syntax item. */
    readonly value: Scalar;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
}
/** `quasis.length === parts.length + 1`. */
 | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "template";
    /** Literal template segments; contains one more segment than parts. */
    readonly quasis: readonly string[];
    /** Interpolated template expressions in source order. */
    readonly parts: readonly Expr[];
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
}
/** `$name`; `name` excludes the `$`. */
 | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "state";
    /** Identifier, property, method or callee name; state names exclude the dollar prefix. */
    readonly name: string;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
}
/** A named value or an arrow parameter. */
 | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "ident";
    /** Identifier, property, method or callee name; state names exclude the dollar prefix. */
    readonly name: string;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
} | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "list";
    /** Ordered list-element expressions. */
    readonly items: readonly Expr[];
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
} | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "object";
    /** Object entries in source insertion order. */
    readonly entries: readonly {
        /** Object key or stable source-derived native component identity. */
        readonly key: string;
        /** Literal value or unevaluated expression belonging to this syntax item. */
        readonly value: Expr;
        /** One-based UTF-16 source position used for diagnostics and component identity. */
        readonly pos: Pos;
    }[];
    /** Source location of this syntax item. */
    readonly pos: Pos;
} | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "unary";
    /** Whitelisted pure operator spelling. */
    readonly op: "!" | "-";
    /** Unevaluated prefix operand. */
    readonly operand: Expr;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
} | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "binary";
    /** Whitelisted pure operator spelling. */
    readonly op: BinaryOperator;
    /** Left operand; evaluated before the right operand. */
    readonly left: Expr;
    /** Right operand; logical operators evaluate this lazily. */
    readonly right: Expr;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
} | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "conditional";
    /** Conditional test expression. */
    readonly test: Expr;
    /** Branch evaluated for a truthy test. */
    readonly then: Expr;
    /** Branch evaluated for a false test. */
    readonly else: Expr;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
}
/** `object.name`; `pos` is the property name. */
 | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "member";
    /** Receiver expression for a property, index or method access. */
    readonly object: Expr;
    /** Identifier, property, method or callee name; state names exclude the dollar prefix. */
    readonly name: string;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
} | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "index";
    /** Receiver expression for a property, index or method access. */
    readonly object: Expr;
    /** Index expression; lists and strings use numbers, objects use text. */
    readonly index: Expr;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
}
/** `name(...)`: a component, action or function; `pos` is the name (the node key). */
 | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "call";
    /** Identifier without a state prefix. */
    readonly name: string;
    /** Positional call arguments in source order. */
    readonly args: readonly Expr[];
    /** Top-level definitions in source insertion order. */
    readonly named: readonly NamedArg[];
    /** Source location of this syntax item. */
    readonly pos: Pos;
}
/** `object.name(...)`; `pos` is the method name. */
 | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "method";
    /** Receiver expression for a property, index or method access. */
    readonly object: Expr;
    /** Identifier without a state prefix. */
    readonly name: string;
    /** Positional call arguments in source order. */
    readonly args: readonly Expr[];
    /** Top-level definitions in source insertion order. */
    readonly named: readonly NamedArg[];
    /** Source location of this syntax item. */
    readonly pos: Pos;
} | {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    readonly type: "arrow";
    /** Lexical callback parameter names. */
    readonly params: readonly string[];
    /** Pure function body; never platform or JavaScript code. */
    readonly body: Expr;
    /** One-based UTF-16 source position used for diagnostics and component identity. */
    readonly pos: Pos;
};
/** Component/action/pure-function call expression. */
export type CallExpr = Extract<Expr, {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    type: "call";
}>;
/** Lexically scoped list callback or named-function body. */
export type ArrowExpr = Extract<Expr, {
    /** Syntax or interpreter-value discriminator; determines the other fields. */
    type: "arrow";
}>;
/** Source-ordered state initializer or top-level named value. */
export interface Decl {
    /** Identifier without a state prefix. */
    readonly name: string;
    /** Unevaluated expression. */
    readonly value: Expr;
    /** Source location of this syntax item. */
    readonly pos: Pos;
}
/** A state declaration, named declaration or bare render root. */
export type Statement = ({
    /** Statement or interpreter-value discriminator. */
    readonly kind: "state";
} & Decl) | ({
    /** Statement or interpreter-value discriminator. */
    readonly kind: "named";
} & Decl) | {
    /** Statement or interpreter-value discriminator. */
    readonly kind: "root";
    /** Literal value or unevaluated expression belonging to this syntax item. */
    readonly value: Expr;
};
/** A validated program. Only `compile` creates one. */
export interface Program {
    /** Serialized statement bytes, retained so evaluation can recheck a lowered byte budget. */
    readonly serializedBytes: number;
    /** Local state initializers in source declaration order. */
    readonly states: readonly Decl[];
    /** Top-level definitions in source insertion order. */
    readonly named: ReadonlyMap<string, Decl>;
    /** Bare expressions in source order, or the named value `root`. */
    readonly roots: readonly Expr[];
}
