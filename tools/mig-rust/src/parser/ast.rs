/// Abstract Syntax Tree for MIG .defs files

/// Top-level subsystem definition
#[derive(Debug, Clone)]
pub struct Subsystem {
    pub name: String,
    pub base: u32,
    pub modifiers: Vec<SubsystemMod>,
    pub statements: Vec<Statement>,
}

/// Subsystem modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemMod {
    KernelUser,
    KernelServer,
}

/// Top-level statement
#[derive(Debug, Clone)]
pub enum Statement {
    TypeDecl(TypeDecl),
    Routine(Routine),
    SimpleRoutine(Routine),
    Import(Import),
    Skip,
}

/// Type declaration: type name = typespec [annotations];
#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: String,
    pub spec: TypeSpec,
    pub annotations: TypeAnnotations,
}

/// Type annotations (ctype, intran, outtran, etc.)
#[derive(Debug, Clone, Default)]
pub struct TypeAnnotations {
    pub ctype: Option<String>,
    pub cusertype: Option<String>,
    pub cservertype: Option<String>,
    pub intran: Option<(String, String)>, // (type, function)
    pub intranpayload: Option<(String, String)>,
    pub outtran: Option<(String, String)>,
    pub destructor: Option<String>,
}

/// Type specification
#[derive(Debug, Clone)]
pub enum TypeSpec {
    /// Basic type (identifier)
    Basic(String),

    /// Array type
    Array {
        size: ArraySize,
        element: Box<TypeSpec>,
    },

    /// Pointer type (^type)
    Pointer(Box<TypeSpec>),

    /// Struct type
    Struct(Vec<StructField>),

    /// Struct array: struct[count] of type
    StructArray {
        count: u32,
        element: Box<TypeSpec>,
    },

    /// C string
    CString { max_size: Option<u32>, varying: bool },
}

/// Array size specification
#[derive(Debug, Clone, Copy)]
pub enum ArraySize {
    Fixed(u32),
    Variable,
    VariableWithMax(u32),
}

/// Struct field
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub field_type: TypeSpec,
}

/// Routine definition
#[derive(Debug, Clone)]
pub struct Routine {
    pub name: String,
    pub kind: RoutineKind,
    pub args: Vec<Argument>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutineKind {
    Routine,
    SimpleRoutine,
}

/// Routine argument
#[derive(Debug, Clone)]
pub struct Argument {
    pub name: String,
    pub direction: Direction,
    pub arg_type: TypeSpec,
    pub flags: IpcFlags,
}

/// Argument direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
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
}

/// IPC flags for arguments
#[derive(Debug, Clone, Default)]
pub struct IpcFlags {
    pub is_long: Option<bool>,
    pub dealloc: Option<DeallocMode>,
    pub server_copy: bool,
    pub count_in_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeallocMode {
    Dealloc,
    NotDealloc,
}

/// Import statement
#[derive(Debug, Clone)]
pub struct Import {
    pub kind: ImportKind,
    pub file: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Normal,  // import
    User,    // uimport
    Server,  // simport
}
