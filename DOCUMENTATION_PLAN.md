# Documentation Organization Plan

## Current State Analysis

### Documentation Categories Found

#### 1. **Core Architecture & Design** (Keep at Top Level)
- `MACH_R_ARCHITECTURE.md` - Comprehensive architecture overview
- `REAL_MACH_R_DESIGN.md` - Design decisions
- `MACH_PORT_SEMANTICS.md` - Port system details
- `ENGINEERING_PLAN.md` - Engineering approach
- `CLAUDE.md` - Project guidelines

#### 2. **Project Status & Planning**
- `PROJECT_REALITY.md` - Honest assessment
- `PROJECT_PLAN.md` - Initial project plan
- `REAL_ROADMAP.md` - Realistic roadmap
- `ROADMAP.md` - Original roadmap
- `WORKING_STATUS.md` - Current status
- `TODO.md` - Task tracking

#### 3. **Developer Documentation** (docs/)
- `docs/ARCHITECTURE.md` - Duplicate of MACH_R_ARCHITECTURE.md
- `docs/CLEAN_ROOM.md` - Clean room development
- `docs/GDB.md` - Debugging guide
- `docs/ADDING_MODULE.md` - Module development
- `docs/MIG.md` - MIG system
- `docs/STRUCTURE.md` - Project structure
- `docs/IMAGES.md` - Disk image creation

#### 4. **MIG Tool Documentation** (tools/mig-rust/)
- Design documents (PHASE2_DESIGN.md, etc.)
- Session notes (SESSION_*.md)
- Testing results
- Implementation guides

#### 5. **Analysis & Reports** (analysis/, reports/)
- MIG analysis
- OSFMK audit reports
- Build analysis

#### 6. **Archived/Historical** (archive/)
- Conflicting documentation
- Old status reports

## Proposed Organization Structure

```
/
├── README.md                          # Main project README (NEW - comprehensive)
├── CONTRIBUTING.md                    # Contribution guidelines (NEW)
├── LICENSE                            # MIT License (VERIFY EXISTS)
├── ARCHITECTURE.md                    # Rename from MACH_R_ARCHITECTURE.md
├── ROADMAP.md                         # Keep consolidated roadmap
├── CHANGELOG.md                       # Version history (NEW)
│
├── docs/
│   ├── INDEX.md                       # Documentation index (NEW)
│   │
│   ├── architecture/
│   │   ├── overview.md                # High-level architecture
│   │   ├── ipc-system.md              # Port semantics (from MACH_PORT_SEMANTICS.md)
│   │   ├── memory-management.md       # VM subsystem
│   │   ├── task-threading.md          # Task/thread model
│   │   └── design-decisions.md        # From REAL_MACH_R_DESIGN.md
│   │
│   ├── development/
│   │   ├── building.md                # Build instructions
│   │   ├── debugging.md               # From GDB.md
│   │   ├── testing.md                 # Testing guide
│   │   ├── adding-modules.md          # From ADDING_MODULE.md
│   │   ├── clean-room.md              # From CLEAN_ROOM.md
│   │   └── code-style.md              # Coding standards
│   │
│   ├── tools/
│   │   ├── mig/
│   │   │   ├── README.md              # MIG overview
│   │   │   ├── design.md              # Consolidated design
│   │   │   ├── usage.md               # How to use
│   │   │   └── implementation.md      # Implementation details
│   │   └── disk-images.md             # From IMAGES.md
│   │
│   ├── project/
│   │   ├── status.md                  # Current status
│   │   ├── reality-check.md           # From PROJECT_REALITY.md
│   │   ├── engineering-plan.md        # From ENGINEERING_PLAN.md
│   │   └── project-plan.md            # From PROJECT_PLAN.md
│   │
│   └── book/                          # Keep existing mdBook
│       └── ...
│
├── tools/
│   ├── mig-rust/
│   │   ├── README.md                  # MIG tool README
│   │   ├── docs/
│   │   │   ├── design/                # Design documents
│   │   │   ├── sessions/              # Development sessions (archived)
│   │   │   └── testing/               # Testing documentation
│   │   └── ...
│   └── ...
│
├── analysis/                          # Keep as-is
│   └── ...
│
├── reports/                           # Keep as-is
│   └── ...
│
└── archive/                           # Historical/deprecated docs
    ├── conflicting_docs/
    ├── old_roadmaps/                  # Move old roadmaps here
    └── session_notes/                 # Move SESSION_*.md here
```

## Actions Required

### Phase 1: Reorganize Existing Documentation
1. Move duplicate/conflicting docs to archive/
2. Consolidate similar documents
3. Create new directory structure
4. Move files to appropriate locations

### Phase 2: Create New Documentation
1. **README.md** - Comprehensive project README
2. **CONTRIBUTING.md** - Developer guidelines
3. **docs/INDEX.md** - Documentation navigation
4. **docs/architecture/** - Split architecture docs
5. **docs/development/** - Developer guides
6. **docs/tools/mig/** - Consolidated MIG docs

### Phase 3: Clean Up Tool Documentation
1. Move MIG session notes to archive
2. Consolidate design documents
3. Create clear MIG user guide

### Phase 4: GitHub Publication Checklist
1. Verify LICENSE file exists
2. Add .github/ directory with templates
3. Add badges to README
4. Ensure all links work
5. Add contributing guidelines
6. Create initial CHANGELOG

## Documentation Consolidation

### Files to Archive
- `archive/old_roadmaps/ROADMAP.md` (if superseded)
- `archive/session_notes/SESSION_*.md` (from tools/mig-rust/)
- `archive/old_status/WORKING_STATUS.md` (keep latest in docs/project/)

### Files to Merge/Consolidate
- `MACH_R_ARCHITECTURE.md` + `docs/ARCHITECTURE.md` → `ARCHITECTURE.md`
- `MACH_PORT_SEMANTICS.md` → `docs/architecture/ipc-system.md`
- `REAL_MACH_R_DESIGN.md` → `docs/architecture/design-decisions.md`
- MIG design docs → `docs/tools/mig/design.md`

### Files to Keep at Top Level
- `README.md` (NEW - comprehensive)
- `CONTRIBUTING.md` (NEW)
- `ARCHITECTURE.md` (consolidated)
- `ROADMAP.md` (current roadmap)
- `LICENSE`
- `CHANGELOG.md` (NEW)
- `CLAUDE.md` (project-specific instructions)

## Success Criteria

- [ ] Single source of truth for each topic
- [ ] Clear navigation through documentation
- [ ] Professional GitHub presentation
- [ ] Easy onboarding for new contributors
- [ ] No duplicate or conflicting information
- [ ] All links functional
- [ ] Proper categorization
- [ ] Historical context preserved in archive/
