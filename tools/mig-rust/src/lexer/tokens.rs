/// Token types in MIG .defs files
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A keyword (subsystem, routine, type, etc.)
    Keyword(Keyword),
    /// An identifier (variable, type name, etc.)
    Identifier(String),
    /// A number literal
    Number(u32),
    /// A string literal
    String(String),
    /// A symbol (punctuation, operators)
    Symbol(Symbol),
    /// A preprocessor directive
    Preprocessor(String),
    /// A comment (skipped in parsing)
    Comment,
}

/// MIG keywords (case-insensitive in original MIG)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    // Subsystem declaration
    Subsystem,
    KernelUser,
    KernelServer,

    // Routine types
    Routine,
    SimpleRoutine,
    Procedure,
    SimpleProcedure,

    // Type system
    Type,
    Array,
    Of,
    Struct,
    CString,

    // Type annotations
    CType,
    CUserType,
    CServerType,
    InTran,
    InTranPayload,
    OutTran,
    Destructor,

    // Import directives
    Import,
    UImport,
    SImport,

    // Special directives
    RCSId,
    Skip,

    // Argument directions
    In,
    Out,
    InOut,
    RequestPort,
    ReplyPort,
    SReplyPort,
    UReplyPort,
    WaitTime,
    MsgOption,
    MsgSeqno,

    // IPC flags
    IsLong,
    IsNotLong,
    Dealloc,
    NotDealloc,
    ServerCopy,
    CountInOut,
}

impl Keyword {
    /// Get the string representation of the keyword
    pub fn as_str(&self) -> &'static str {
        match self {
            Keyword::Subsystem => "subsystem",
            Keyword::KernelUser => "KernelUser",
            Keyword::KernelServer => "KernelServer",
            Keyword::Routine => "routine",
            Keyword::SimpleRoutine => "simpleroutine",
            Keyword::Procedure => "procedure",
            Keyword::SimpleProcedure => "simpleprocedure",
            Keyword::Type => "type",
            Keyword::Array => "array",
            Keyword::Of => "of",
            Keyword::Struct => "struct",
            Keyword::CString => "c_string",
            Keyword::CType => "ctype",
            Keyword::CUserType => "cusertype",
            Keyword::CServerType => "cservertype",
            Keyword::InTran => "intran",
            Keyword::InTranPayload => "intranpayload",
            Keyword::OutTran => "outtran",
            Keyword::Destructor => "destructor",
            Keyword::Import => "import",
            Keyword::UImport => "uimport",
            Keyword::SImport => "simport",
            Keyword::RCSId => "RCSId",
            Keyword::Skip => "skip",
            Keyword::In => "in",
            Keyword::Out => "out",
            Keyword::InOut => "inout",
            Keyword::RequestPort => "requestport",
            Keyword::ReplyPort => "replyport",
            Keyword::SReplyPort => "sreplyport",
            Keyword::UReplyPort => "ureplyport",
            Keyword::WaitTime => "waittime",
            Keyword::MsgOption => "msgoption",
            Keyword::MsgSeqno => "msgseqno",
            Keyword::IsLong => "IsLong",
            Keyword::IsNotLong => "IsNotLong",
            Keyword::Dealloc => "Dealloc",
            Keyword::NotDealloc => "NotDealloc",
            Keyword::ServerCopy => "ServerCopy",
            Keyword::CountInOut => "CountInOut",
        }
    }
}

impl std::fmt::Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Symbols and punctuation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    Colon,        // :
    Semicolon,    // ;
    Comma,        // ,
    LeftParen,    // (
    RightParen,   // )
    LeftBracket,  // [
    RightBracket, // ]
    LeftBrace,    // {
    RightBrace,   // }
    Equals,       // =
    Star,         // *
    Caret,        // ^
    Tilde,        // ~
    Plus,         // +
    Minus,        // -
    Slash,        // /
    Pipe,         // |
    Ampersand,    // &
    LessThan,     // <
    GreaterThan,  // >
    Dot,          // .
}

impl Symbol {
    pub fn as_char(&self) -> char {
        match self {
            Symbol::Colon => ':',
            Symbol::Semicolon => ';',
            Symbol::Comma => ',',
            Symbol::LeftParen => '(',
            Symbol::RightParen => ')',
            Symbol::LeftBracket => '[',
            Symbol::RightBracket => ']',
            Symbol::LeftBrace => '{',
            Symbol::RightBrace => '}',
            Symbol::Equals => '=',
            Symbol::Star => '*',
            Symbol::Caret => '^',
            Symbol::Tilde => '~',
            Symbol::Plus => '+',
            Symbol::Minus => '-',
            Symbol::Slash => '/',
            Symbol::Pipe => '|',
            Symbol::Ampersand => '&',
            Symbol::LessThan => '<',
            Symbol::GreaterThan => '>',
            Symbol::Dot => '.',
        }
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_char())
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Keyword(k) => write!(f, "Keyword({})", k),
            Token::Identifier(s) => write!(f, "Identifier({})", s),
            Token::Number(n) => write!(f, "Number({})", n),
            Token::String(s) => write!(f, "String(\"{}\")", s),
            Token::Symbol(s) => write!(f, "Symbol({})", s),
            Token::Preprocessor(s) => write!(f, "Preprocessor({})", s),
            Token::Comment => write!(f, "Comment"),
        }
    }
}
