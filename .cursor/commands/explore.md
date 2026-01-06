# /explore - Deep Codebase Analysis Command

## Command Overview

The `/explore` command performs exhaustive codebase analysis to create a comprehensive knowledge base for agent operations. This command prioritizes thoroughness over efficiency, building detailed documentation of code structure, relationships, and patterns.

## Command Syntax

```
/explore [path] [options]
```

### Options

- `--depth`: Maximum directory depth (default: unlimited)
- `--focus`: Specific areas to emphasize (e.g., "api", "database", "frontend")
- `--output`: Output directory for documentation (default: `.agent-docs/`)
- `--update`: Update existing documentation rather than regenerate

## Execution Phases

### Phase 1: Structure Mapping

```bash
# Initial scan - build complete file tree
find . -type f -name "*.{js,jsx,ts,tsx,py,rb,go,java,c,cpp,rs,sql,yaml,yml,json,xml,md}" | head -1000 > file_manifest.txt

# Categorize by extension and directory patterns
awk -F'/' '{print $2}' file_manifest.txt | sort | uniq -c > directory_summary.txt
find . -type f | sed 's/.*\.//' | sort | uniq -c > extension_summary.txt

# Identify framework signatures
grep -l "package.json\|Gemfile\|go.mod\|Cargo.toml\|pom.xml\|requirements.txt" . -r --include="*" 2>/dev/null
```

### Phase 2: Dependency Analysis

```bash
# Package dependencies
if [ -f package.json ]; then jq '.dependencies, .devDependencies' package.json; fi
if [ -f Gemfile.lock ]; then grep "^    " Gemfile.lock | awk '{print $1}' | sort -u; fi
if [ -f go.mod ]; then grep "require" go.mod; fi
if [ -f requirements.txt ]; then cat requirements.txt; fi

# Import/require analysis
grep -h "^import\|^require\|^from.*import" -r . --include="*.{py,js,ts,rb,go}" | sort | uniq -c | sort -rn | head -50
```

### Phase 3: Pattern Recognition

```bash
# API endpoints
grep -r "app\.\(get\|post\|put\|delete\|patch\)\|@app\.route\|@GetMapping\|@PostMapping" . --include="*.{js,py,java,rb}"

# Database models/schemas
grep -r "CREATE TABLE\|Schema\|Model\|Entity" . --include="*.{sql,js,py,rb,java}"

# Configuration patterns
find . -name "*.env*" -o -name "*config*" -o -name "*settings*" | grep -v node_modules
```

## Documentation Generation

### 1. Project Overview (PROJECT_OVERVIEW.md)

```markdown
# Project Overview

## Technology Stack
- Primary Language: [detected]
- Framework: [detected]
- Database: [detected]
- Build System: [detected]

## Architecture Type
- [ ] Monolithic
- [ ] Microservices
- [ ] Serverless
- [ ] Hybrid

## Key Directories
| Directory | Purpose | Key Files |
|-----------|---------|-----------|
| /src | Main application code | index.ts, app.ts |
| /api | API endpoints | routes/, controllers/ |
| /models | Data models | user.model.ts |
| /services | Business logic | auth.service.ts |
| /utils | Shared utilities | logger.ts, validator.ts |
```

### 2. File Purpose Index (FILE_PURPOSE_INDEX.md)

```markdown
# File Purpose Index

## Core Application Files
| File Path | Purpose | Dependencies | Dependents |
|-----------|---------|--------------|------------|
| src/index.ts | Application entry point | express, dotenv | - |
| src/app.ts | Express app configuration | middleware/*, routes/* | index.ts |
| src/routes/auth.ts | Authentication routes | controllers/auth | app.ts |

## Configuration Files
| File | Purpose | Modified Frequency |
|------|---------|-------------------|
| .env | Environment variables | Rarely |
| tsconfig.json | TypeScript configuration | Rarely |
| package.json | Dependencies & scripts | Frequently |
```

### 3. Relationship Map (RELATIONSHIP_MAP.md)

```markdown
# Component Relationship Map

## Import Graph
```

index.ts
├── app.ts
│   ├── middleware/
│   │   ├── auth.middleware.ts
│   │   └── error.middleware.ts
│   └── routes/
│       ├── auth.routes.ts
│       │   └── controllers/auth.controller.ts
│       │       └── services/auth.service.ts
│       └── user.routes.ts
│           └── controllers/user.controller.ts
│               ├── services/user.service.ts
│               └── models/user.model.ts

```

## Cross-Component Dependencies
- auth.service.ts → user.model.ts (User authentication)
- user.service.ts → auth.middleware.ts (Protected routes)
- error.middleware.ts → logger.utils.ts (Error logging)
```

### 4. Code Patterns (CODE_PATTERNS.md)

```markdown
# Identified Code Patterns

## Authentication Pattern
- Location: src/middleware/auth.ts
- Pattern: JWT token validation
- Usage: All protected routes

## Error Handling Pattern
- Location: src/middleware/error.ts
- Pattern: Centralized error handler
- Response Format: { error: string, code: number }

## Database Query Pattern
- Location: src/services/*.service.ts
- Pattern: Repository pattern with TypeORM
- Transaction Support: Yes

## API Response Pattern
- Standard Format: { success: boolean, data?: any, error?: string }
- Status Codes: Consistent HTTP status usage
```

### 5. Working Notes (WORKING_NOTES.md)

```markdown
# Agent Working Notes

## Critical Files
1. **src/config/database.ts** - Database connection, modify with caution
2. **src/middleware/auth.ts** - Authentication logic, security-critical
3. **src/utils/validator.ts** - Input validation, affects all endpoints

## Common Tasks

### Adding New Endpoint
1. Create route file in src/routes/
2. Create controller in src/controllers/
3. Register route in src/app.ts
4. Add tests in tests/

### Modifying Database Schema
1. Update model in src/models/
2. Create migration in migrations/
3. Update seed data if needed
4. Update validation schemas

## Known Issues
- Rate limiting not implemented on /api/search
- Password reset emails queued but not sent in dev
- Large file uploads timeout after 30s

## Performance Considerations
- Database queries in loops in user.service.ts:L45
- N+1 query issue in posts.controller.ts:L78
- Consider caching for frequently accessed configs
```

## Implementation Script

```bash
#!/bin/bash
# explore-codebase.sh

PROJECT_ROOT="${1:-.}"
DOC_DIR="${2:-.agent-docs}"

echo "Starting deep codebase exploration..."

# Create documentation directory
mkdir -p "$DOC_DIR"

# Phase 1: Structure Analysis
echo "Phase 1: Analyzing structure..."
find "$PROJECT_ROOT" -type f \( -name "*.js" -o -name "*.ts" -o -name "*.py" -o -name "*.rb" -o -name "*.go" \) | \
    grep -v node_modules | grep -v ".git" > "$DOC_DIR/files.txt"

# Phase 2: Generate file purposes
echo "Phase 2: Analyzing file purposes..."
while IFS= read -r file; do
    echo "## $file" >> "$DOC_DIR/FILE_PURPOSE_INDEX.md"
    head -20 "$file" | grep -E "^//|^#|^/\*|^\*" >> "$DOC_DIR/FILE_PURPOSE_INDEX.md" || echo "No header comments" >> "$DOC_DIR/FILE_PURPOSE_INDEX.md"
    echo "" >> "$DOC_DIR/FILE_PURPOSE_INDEX.md"
done < "$DOC_DIR/files.txt"

# Phase 3: Relationship mapping
echo "Phase 3: Mapping relationships..."
for file in $(cat "$DOC_DIR/files.txt"); do
    echo "### $file" >> "$DOC_DIR/RELATIONSHIP_MAP.md"
    grep -E "^import|^require|^from .* import" "$file" 2>/dev/null >> "$DOC_DIR/RELATIONSHIP_MAP.md" || true
    echo "" >> "$DOC_DIR/RELATIONSHIP_MAP.md"
done

# Phase 4: Pattern detection
echo "Phase 4: Detecting patterns..."
grep -r "class.*Controller\|class.*Service\|class.*Model" "$PROJECT_ROOT" --include="*.{js,ts,py,rb}" | \
    awk -F: '{print $1}' | sort -u > "$DOC_DIR/pattern_files.txt"

echo "Exploration complete. Documentation generated in $DOC_DIR/"
```

## Agent Usage Guidelines

### When to Use /explore

1. **Initial Project Onboarding** - First interaction with a codebase
2. **Major Refactoring** - Before structural changes
3. **Debugging Complex Issues** - When relationships are unclear
4. **Documentation Updates** - Periodic knowledge base refresh

### Reading the Documentation

1. Start with PROJECT_OVERVIEW.md for context
2. Use FILE_PURPOSE_INDEX.md to locate specific functionality
3. Reference RELATIONSHIP_MAP.md when modifying interconnected components
4. Check WORKING_NOTES.md for gotchas and common tasks

### Maintaining Documentation

- Run `/explore --update` after significant changes
- Manually update WORKING_NOTES.md with discoveries
- Flag outdated sections for regeneration

## Performance Considerations

### Token Usage Expectations

- Small project (<1000 files): ~50k tokens
- Medium project (1000-5000 files): ~200k tokens  
- Large project (5000+ files): ~500k+ tokens

### Optimization Strategies

1. Use `--focus` to limit scope when possible
2. Cache unchanged sections with `--update`
3. Exclude test files and dependencies unless needed
4. Prioritize core application code over libraries

## Error Handling

### Common Issues

```bash
# Permission denied
sudo chmod -R 755 .agent-docs/

# Out of memory
/explore --depth 3  # Limit depth

# Circular dependencies detected
# Check RELATIONSHIP_MAP.md for circular imports
```

## Integration with Other Commands

### Complementary Commands

- `/analyze` - Quick analysis of specific files
- `/test` - Run tests after exploration
- `/refactor` - Use exploration data for refactoring
- `/document` - Generate user-facing documentation

### Data Sharing

Exploration data is stored in `.agent-docs/` and can be referenced by other commands:

```bash
EXPLORATION_DATA=".agent-docs/FILE_PURPOSE_INDEX.md"
```
