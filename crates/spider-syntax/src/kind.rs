//! Every token and node kind in the Spider concrete syntax tree.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxKind {
    // ----- trivia tokens -----
    Whitespace,
    Comment,
    DocComment,

    // ----- literal and name tokens -----
    Ident,
    IntLit,
    FloatLit,
    StrLit,

    // ----- punctuation tokens -----
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
    Arrow,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    // ----- keyword tokens -----
    LetKw,
    VarKw,
    FnKw,
    PublicKw,
    IfKw,
    ElseKw,
    ForKw,
    InKw,
    ToKw,
    WhileKw,
    RepeatKw,
    TimesKw,
    MatchKw,
    TryKw,
    UseKw,
    ReturnKw,
    SayKw,
    AskKw,
    RecordKw,
    ChoiceKw,
    ShapeKw,
    TestKw,
    SpawnKw,
    DoKw,
    TogetherKw,
    AndKw,
    OrKw,
    NotKw,
    TrueKw,
    FalseKw,
    OfKw,
    WhereKw,
    IsKw,

    // ----- layout tokens (Indent/Dedent are zero-width) -----
    Newline,
    Indent,
    Dedent,
    Eof,

    // ----- error token -----
    ErrorToken,

    // ----- nodes -----
    SourceFile,
    Block,
    UseDecl,
    FnDecl,
    FnSig,
    ParamList,
    Param,
    RetType,
    WhereClause,
    RecordDecl,
    FieldDecl,
    ChoiceDecl,
    VariantDecl,
    ShapeDecl,
    TestDecl,
    LetStmt,
    VarStmt,
    AssignStmt,
    ExprStmt,
    SayStmt,
    ReturnStmt,
    SpawnStmt,
    IfStmt,
    ElseClause,
    ForStmt,
    WhileStmt,
    RepeatStmt,
    DoTogetherStmt,
    MatchStmt,
    MatchArm,
    Pattern,
    TypeRef,
    BinaryExpr,
    UnaryExpr,
    RangeExpr,
    CallExpr,
    ArgList,
    FieldExpr,
    IndexExpr,
    ParenExpr,
    ListExpr,
    MapExpr,
    MapEntry,
    AskExpr,
    TryExpr,
    Literal,
    NameRef,
    ErrorNode,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace | SyntaxKind::Comment | SyntaxKind::DocComment
        )
    }

    /// True for node kinds that represent an expression.
    pub fn is_expr(self) -> bool {
        matches!(
            self,
            SyntaxKind::BinaryExpr
                | SyntaxKind::UnaryExpr
                | SyntaxKind::RangeExpr
                | SyntaxKind::CallExpr
                | SyntaxKind::FieldExpr
                | SyntaxKind::IndexExpr
                | SyntaxKind::ParenExpr
                | SyntaxKind::ListExpr
                | SyntaxKind::MapExpr
                | SyntaxKind::AskExpr
                | SyntaxKind::TryExpr
                | SyntaxKind::Literal
                | SyntaxKind::NameRef
        )
    }

    pub fn keyword(text: &str) -> Option<SyntaxKind> {
        use SyntaxKind::*;
        Some(match text {
            "let" => LetKw,
            "var" => VarKw,
            "fn" => FnKw,
            "public" => PublicKw,
            "if" => IfKw,
            "else" => ElseKw,
            "for" => ForKw,
            "in" => InKw,
            "to" => ToKw,
            "while" => WhileKw,
            "repeat" => RepeatKw,
            "times" => TimesKw,
            "match" => MatchKw,
            "try" => TryKw,
            "use" => UseKw,
            "return" => ReturnKw,
            "say" => SayKw,
            "ask" => AskKw,
            "record" => RecordKw,
            "choice" => ChoiceKw,
            "shape" => ShapeKw,
            "test" => TestKw,
            "spawn" => SpawnKw,
            "do" => DoKw,
            "together" => TogetherKw,
            "and" => AndKw,
            "or" => OrKw,
            "not" => NotKw,
            "true" => TrueKw,
            "false" => FalseKw,
            "of" => OfKw,
            "where" => WhereKw,
            "is" => IsKw,
            _ => return None,
        })
    }

    /// Short human name, used in "expected …" diagnostics.
    pub fn describe(self) -> &'static str {
        use SyntaxKind::*;
        match self {
            Ident => "a name",
            IntLit => "a whole number",
            FloatLit => "a decimal number",
            StrLit => "some text in quotes",
            LParen => "`(`",
            RParen => "`)`",
            LBracket => "`[`",
            RBracket => "`]`",
            LBrace => "`{`",
            RBrace => "`}`",
            Comma => "`,`",
            Colon => "`:`",
            Dot => "`.`",
            Arrow => "`->`",
            Assign => "`=`",
            Newline => "the end of the line",
            Indent => "an indented block",
            Dedent => "the end of the block",
            InKw => "`in`",
            ToKw => "`to`",
            TimesKw => "`times`",
            TogetherKw => "`together`",
            IsKw => "`is`",
            Eof => "the end of the file",
            _ => "something else",
        }
    }
}
