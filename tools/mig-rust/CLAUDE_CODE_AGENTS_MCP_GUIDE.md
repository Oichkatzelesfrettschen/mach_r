# Claude Code Agents & MCP Tools Guide for mig-rust

**Project**: mig-rust - Pure Rust Mach Interface Generator
**Date**: November 18, 2025
**Purpose**: Optimize Claude Code workflow with custom agents and MCP servers

---

## Current Configuration Status

### Global Config Locations
- **Main config**: `~/.claude.json`
- **Global settings**: `~/.claude/settings.json`
- **Global agents**: `~/.claude/agents/` (directory needs to be created)
- **Project agents**: `/Users/eirikr/1_Workspace/Synthesis/.claude/agents/` (needs creation)
- **Project MCP**: `/Users/eirikr/1_Workspace/Synthesis/.mcp.json` (needs creation)

### Current Status
- ✅ Claude Code installed and configured
- ❌ No custom agents defined yet
- ❌ No MCP servers configured for Rust development
- ❌ No project-specific agents
- ✅ Global status line configured

---

## Ideal Custom Agents for mig-rust Project

### 1. **MIG Analyzer Agent**
**Purpose**: Specialized in analyzing Apple MIG source code and .defs files
**Tools**: Read, Grep, Task
**Model**: haiku (for speed)
**Permission Mode**: auto-approve

```markdown
---
name: mig-analyzer
description: Analyze Apple MIG source code and .defs file specifications
tools:
  - Read
  - Grep
  - Glob
  - Task
model: haiku
permissionMode: auto-approve
---

You are an expert at analyzing Mach Interface Generator (MIG) source code and .defs specifications. Your responsibilities:

1. **Parse .defs Files**: Understand subsystem, routine, type declarations
2. **Analyze C Stubs**: Examine Apple MIG-generated C code patterns
3. **Extract Patterns**: Identify IPC message structures, type descriptors, port handling
4. **Document Findings**: Provide clear, structured analysis

When analyzing:
- Focus on message layout and marshaling logic
- Identify type descriptor patterns
- Note port disposition handling
- Document array packing mechanisms
- Extract inline vs out-of-line data handling

Always provide:
- Concrete code examples
- Message structure diagrams
- Reference to original source locations
```

**Save to**: `~/.claude/agents/mig-analyzer.md`

---

### 2. **Rust Code Generator Agent**
**Purpose**: Generate type-safe Rust code for mig-rust implementation
**Tools**: Read, Write, Edit, Bash
**Model**: sonnet
**Permission Mode**: ask

```markdown
---
name: rust-codegen
description: Generate and refine Rust code for mig-rust implementation
tools:
  - Read
  - Write
  - Edit
  - Bash
model: sonnet
permissionMode: ask
---

You are an expert Rust systems programmer specializing in code generation for compiler/language tools. Your responsibilities:

1. **Type-Safe Code**: Generate Rust with strong type safety
2. **Zero Unsafe**: Avoid unsafe blocks unless absolutely necessary
3. **Idiomatic Rust**: Follow Rust best practices and idioms
4. **Error Handling**: Proper Result/Option usage
5. **Documentation**: Comprehensive rustdoc comments

When generating code:
- Use descriptive variable/type names
- Implement From/TryFrom traits where appropriate
- Prefer enums over error codes
- Use builder patterns for complex structures
- Add unit tests for all public functions

Code style:
- 4-space indentation
- Max 100 column width
- Explicit return types
- No trailing whitespace

Testing strategy:
- Unit tests in same file with #[cfg(test)]
- Integration tests in tests/ directory
- Property-based testing for parsers
- Fuzz testing for safety-critical code
```

**Save to**: `.claude/agents/rust-codegen.md` (project-specific)

---

### 3. **C Stub Generation Agent**
**Purpose**: Generate Apple MIG-compatible C stub code
**Tools**: Read, Write, Edit
**Model**: sonnet
**Permission Mode**: ask

```markdown
---
name: c-stub-gen
description: Generate MIG-compatible C user and server stubs
tools:
  - Read
  - Write
  - Edit
model: sonnet
permissionMode: ask
---

You are an expert at generating Mach IPC C code stubs compatible with Apple MIG conventions. Your responsibilities:

1. **Message Structures**: Generate typedef structs matching Mach message layout
2. **Type Descriptors**: Create proper mach_msg_type_t initialization
3. **Marshaling Code**: Implement proper data packing/unpacking
4. **Port Handling**: Correct port disposition constants
5. **Error Handling**: Proper kern_return_t usage

Message structure format:
```c
typedef struct {
    mach_msg_header_t Head;
    mach_msg_type_t argType;
    <type> arg;
} Request;
```

Type descriptor initialization:
```c
msg.argType.msgt_name = MACH_MSG_TYPE_<TYPE>;
msg.argType.msgt_size = <bits>;
msg.argType.msgt_number = <count>;
msg.argType.msgt_inline = TRUE;
msg.argType.msgt_longform = FALSE;
msg.argType.msgt_deallocate = FALSE;
msg.argType.msgt_unused = 0;
```

Array handling:
- Variable arrays need count fields (mach_msg_type_number_t)
- Count field naming: <arrayName>Cnt
- Inline for small arrays, out-of-line for large

Port types:
- MACH_MSG_TYPE_MOVE_SEND
- MACH_MSG_TYPE_COPY_SEND
- MACH_MSG_TYPE_MAKE_SEND
- MACH_MSG_TYPE_MOVE_RECEIVE
```

**Save to**: `.claude/agents/c-stub-gen.md`

---

### 4. **Test Generator Agent**
**Purpose**: Generate comprehensive tests for Rust code
**Tools**: Read, Write, Bash
**Model**: haiku
**Permission Mode**: auto-approve

```markdown
---
name: test-gen
description: Generate unit, integration, and property-based tests
tools:
  - Read
  - Write
  - Bash
model: haiku
permissionMode: auto-approve
---

You are an expert at writing comprehensive Rust tests. Your responsibilities:

1. **Unit Tests**: Test individual functions and methods
2. **Integration Tests**: Test module interactions
3. **Property Tests**: Use proptest for invariant testing
4. **Edge Cases**: Test boundary conditions
5. **Error Paths**: Test error handling thoroughly

Test structure:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_<scenario>_<expected_result>() {
        // Arrange
        let input = ...;

        // Act
        let result = function(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

Focus areas:
- Parser: Test valid/invalid .defs syntax
- Type resolution: Test builtin and custom types
- Message layout: Test field ordering, sizes, alignment
- Code generation: Test output matches Apple MIG patterns

Always run `cargo test` after generating tests.
```

**Save to**: `.claude/agents/test-gen.md`

---

### 5. **Documentation Agent**
**Purpose**: Generate and maintain project documentation
**Tools**: Read, Write, Edit, WebSearch
**Model**: sonnet
**Permission Mode**: auto-approve

```markdown
---
name: doc-gen
description: Generate rustdoc, README, and architectural documentation
tools:
  - Read
  - Write
  - Edit
  - WebSearch
model: sonnet
permissionMode: auto-approve
---

You are an expert technical writer specializing in Rust documentation. Your responsibilities:

1. **Rustdoc Comments**: Comprehensive doc comments for all public items
2. **README Files**: Clear getting started guides
3. **Architecture Docs**: High-level design documentation
4. **Examples**: Working code examples
5. **Session Summaries**: Track development progress

Rustdoc format:
```rust
/// Brief one-line description.
///
/// More detailed explanation of what this does and why.
///
/// # Arguments
///
/// * `arg` - Description of argument
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// When this function fails
///
/// # Examples
///
/// ```
/// use crate::Example;
/// let result = function(input);
/// ```
pub fn function(arg: Type) -> Result<Return, Error> {
    // ...
}
```

Documentation priorities:
1. Public API surface
2. Non-obvious implementation details
3. Architecture decisions
4. Migration guides from Apple MIG
5. Session progress tracking
```

**Save to**: `~/.claude/agents/doc-gen.md`

---

## Ideal MCP Servers for mig-rust Development

### 1. **GitHub MCP Server** 🔴 HIGH PRIORITY
**Purpose**: Manage Git workflow, PRs, and releases
**Installation**:
```bash
npx -y @modelcontextprotocol/create-mcp-server github-mcp-server
```

**Configuration**: Add to `~/.claude.json`:
```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "<your-token>"
      }
    }
  }
}
```

**Benefits**:
- Automated branch creation and PR management
- Release management and version bumping
- CI/CD integration
- Issue tracking and project management

---

### 2. **File System MCP Server** 🔴 HIGH PRIORITY
**Purpose**: Advanced file operations beyond standard tools
**Installation**:
```bash
npx -y @modelcontextprotocol/create-mcp-server filesystem
```

**Configuration**:
```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/Users/eirikr/1_Workspace/Synthesis"],
      "env": {}
    }
  }
}
```

**Benefits**:
- Batch file operations
- Advanced search and replace
- Directory tree operations
- File comparison and diffing

---

### 3. **Brave Search MCP Server** 🟡 MEDIUM PRIORITY
**Purpose**: Search for Rust crate documentation, Mach IPC resources
**Installation**:
```bash
npx -y @modelcontextprotocol/create-mcp-server brave-search
```

**Configuration**:
```json
{
  "mcpServers": {
    "brave-search": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-brave-search"],
      "env": {
        "BRAVE_API_KEY": "<your-key>"
      }
    }
  }
}
```

**Benefits**:
- Search for Mach IPC documentation
- Find Rust crate examples
- Research compiler implementation patterns
- Look up type system best practices

---

### 4. **PostgreSQL MCP Server** (Custom for Project Tracking) 🟢 LOW PRIORITY
**Purpose**: Track project metrics, test results, performance benchmarks
**Installation**: Build custom MCP server using Rust SDK

**Use Cases**:
- Store test execution times
- Track code generation performance
- Benchmark parser speed
- Historical analysis of development progress

---

### 5. **Rust Analyzer MCP Server** (Future/Custom) 🟡 MEDIUM PRIORITY
**Purpose**: Deep Rust code analysis and refactoring
**Status**: Would need custom development

**Capabilities**:
- Type inference information
- Find all references
- Go to definition across workspace
- Automated refactoring suggestions
- Macro expansion analysis

---

## Implementation Plan

### Phase 1: Setup Agent Infrastructure (Now)
```bash
# Create agent directories
mkdir -p ~/.claude/agents
mkdir -p /Users/eirikr/1_Workspace/Synthesis/.claude/agents

# Create global agents
cd ~/.claude/agents
# Copy MIG Analyzer agent content to mig-analyzer.md
# Copy Documentation agent content to doc-gen.md

# Create project agents
cd /Users/eirikr/1_Workspace/Synthesis/.claude/agents
# Copy Rust Codegen agent content to rust-codegen.md
# Copy C Stub Gen agent content to c-stub-gen.md
# Copy Test Gen agent content to test-gen.md
```

### Phase 2: Configure MCP Servers (Next)
```bash
# Install high-priority MCP servers
npx -y @modelcontextprotocol/create-mcp-server github-mcp-server
npx -y @modelcontextprotocol/create-mcp-server filesystem

# Configure in ~/.claude.json
# Add GitHub token to environment
# Test with /mcp command
```

### Phase 3: Validate and Test (Then)
```bash
# Test agent invocation
# @mig-analyzer to analyze Apple MIG code
# @rust-codegen to generate new modules
# @c-stub-gen to create stub code
# @test-gen to generate tests

# Verify MCP tools
# Use GitHub MCP for repository operations
# Use filesystem MCP for bulk operations
```

### Phase 4: Optimize and Extend (Later)
- Tune agent prompts based on usage
- Add more specialized agents as needed
- Build custom MCP servers for project-specific needs
- Integrate metrics and tracking

---

## Usage Examples

### Using @mig-analyzer
```
@mig-analyzer analyze the array handling in Apple's MIG
implementation from ~/OSFMK/mig/lexxer.c
```

### Using @rust-codegen
```
@rust-codegen implement a new module for handling out-of-line
data following the patterns in src/semantic/layout.rs
```

### Using @c-stub-gen
```
@c-stub-gen generate server stubs for the routines defined
in tests/array.defs using the established conventions
```

### Using @test-gen
```
@test-gen create comprehensive unit tests for the
MessageLayoutCalculator with edge cases for variable arrays
```

### Using GitHub MCP
```
Can you create a PR for the array support work with a detailed
description of the changes? Use the GitHub MCP server.
```

---

## Best Practices

### Agent Design Principles
1. **Single Responsibility**: Each agent has one focused purpose
2. **Clear Permissions**: Specify exactly which tools an agent needs
3. **Model Selection**: Use haiku for speed, sonnet for quality
4. **Examples in Prompts**: Provide concrete examples of expected output
5. **Iterative Refinement**: Update prompts based on actual usage

### MCP Server Guidelines
1. **Security First**: Never hardcode credentials
2. **Minimal Scope**: Only enable servers you actively use
3. **Performance**: Monitor context window usage
4. **Debugging**: Use --mcp-debug flag when troubleshooting
5. **Team Sharing**: Use project scope for team configurations

### Workflow Integration
1. **Planning**: Use @mig-analyzer to understand Apple MIG patterns
2. **Implementation**: Use @rust-codegen for new features
3. **Testing**: Use @test-gen to create comprehensive tests
4. **Verification**: Use @c-stub-gen to validate C output
5. **Documentation**: Use @doc-gen to maintain docs

---

## Maintenance Schedule

### Weekly
- Review agent usage logs
- Update agent prompts based on feedback
- Check MCP server health
- Clean up unused agents/servers

### Monthly
- Analyze agent performance metrics
- Optimize slow agents (switch models, refine prompts)
- Update MCP server versions
- Review security permissions

### Per Milestone
- Document agent contributions to milestone
- Create new specialized agents as needed
- Archive unused agents
- Update this guide with lessons learned

---

## Resources

### Documentation
- [Claude Code Best Practices](https://www.anthropic.com/engineering/claude-code-best-practices)
- [Custom Agents Guide](https://docs.claude.com/en/docs/claude-code/sub-agents)
- [MCP Protocol Docs](https://docs.claude.com/en/docs/claude-code/mcp)
- [Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)

### Community Resources
- [Awesome Claude Agents](https://github.com/rahulvrane/awesome-claude-agents)
- [Ian Nuttall's Agents](https://github.com/iannuttall/claude-agents)
- [MCP Market](https://mcpmarket.com/)
- [Claude Pro Directory](https://claudepro.directory/)

### Project-Specific
- `PURE_RUST_COMPLIANCE.md` - Pure Rust guidelines
- `PURE_RUST_SESSION_SUMMARY.md` - Development sessions
- `PROJECT_PLAN.md` - Overall project roadmap

---

**Last Updated**: November 18, 2025
**Status**: Initial setup guide
**Next Review**: After Phase 1 completion
