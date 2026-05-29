---
name: ai-ready
description: 'Prepare your codebase for AI-assisted development by adding high-quality LLM context files: copilot-instructions.md, .cursorrules, AGENTS.md, and inline code comments. Use when asked to make a project AI-ready, add AI context, or set up GitHub Copilot instructions.'
---

# AI-Ready Codebase Setup

Transform a codebase into an AI-optimized development environment by generating high-quality context files that help LLMs understand the project deeply.

## Overview

Modern AI coding assistants (GitHub Copilot, Cursor, Claude, etc.) work dramatically better when given rich project context. This skill creates the key context files that make any codebase AI-ready.

## Files to Generate

### 1. `.github/copilot-instructions.md`
GitHub Copilot's primary context file. Loaded automatically for all Copilot interactions in the repo.

**Must include:**
- Project purpose and domain (1-2 sentences)
- Tech stack with versions
- Architecture overview
- Key conventions (naming, patterns, error handling)
- File structure guide
- Do's and don'ts for AI suggestions
- Common patterns used in the project

### 2. `.cursorrules` or `.cursor/rules/project.mdc`  
Cursor IDE instructions. Similar to copilot-instructions but optimized for Cursor's context system.

### 3. `AGENTS.md`
Top-level instructions for AI agents that will work on this repo autonomously.

**Must include:**
- Project overview
- Setup commands (install, build, test)
- Development workflow
- Test commands
- Deployment notes
- Critical constraints (e.g., "never modify X", "always run Y before committing")

### 4. `CLAUDE.md` (if using Claude)
Claude-specific context file with the same content as AGENTS.md but formatted for Claude's preferences.

## Workflow

### Step 1: Analyze the Codebase

Read the following files to build context:
- `package.json` / `pyproject.toml` / `Cargo.toml` / etc.
- Existing `README.md`
- Entry point files (`index.ts`, `main.py`, `app.ts`, etc.)
- Config files (`tsconfig.json`, `vite.config.*`, `.eslintrc`, etc.)
- Test files (to understand testing patterns)
- Key source files (to understand conventions)

### Step 2: Extract Key Information

Build a mental model of:
- **Domain**: What problem does this solve?
- **Stack**: Languages, frameworks, major libraries
- **Architecture**: How is code organized? Key patterns?
- **Conventions**: Naming, formatting, error handling patterns
- **Workflows**: How do you build, test, deploy?

### Step 3: Generate Context Files

Create each file with content appropriate to the codebase. Be specific — generic instructions are useless.

## Template: `.github/copilot-instructions.md`

```markdown
# GitHub Copilot Instructions

## Project Overview
[2-3 sentences describing what this project does and its domain]

## Tech Stack
- **Runtime**: [e.g., Node.js 20, Python 3.11, Rust 1.75]
- **Framework**: [e.g., Next.js 14, FastAPI, Axum]
- **Database**: [e.g., PostgreSQL via Prisma, SQLite, None]
- **Testing**: [e.g., Vitest, pytest, cargo test]
- **Key Libraries**: [list 3-5 important libraries]

## Project Structure
```
[key directories and what they contain]
```

## Code Conventions
- [naming convention, e.g., "Use camelCase for variables, PascalCase for components"]
- [error handling pattern]
- [async pattern, e.g., "Use async/await, avoid .then() chains"]
- [import style]
- [comment style]

## Architecture Patterns
- [key pattern 1, e.g., "Repository pattern for data access"]
- [key pattern 2, e.g., "Event-driven state management with Zustand"]

## Do's ✅
- [specific thing AI should do]
- [specific thing AI should do]

## Don'ts ❌  
- [specific thing AI should avoid]
- [specific thing AI should avoid]

## Common Patterns
[Show 1-2 representative code examples that AI should follow]
```

## Template: `AGENTS.md`

```markdown
# Agents Guide

This file guides AI agents working on this repository.

## Project
[1-2 sentence description]

## Quick Start
```bash
# Install
[install command]

# Dev server
[dev command]

# Build
[build command]

# Test
[test command]

# Lint/format
[lint command]
```

## Repository Structure
[key directories and purpose]

## Development Workflow
1. [step 1]
2. [step 2]
3. [step 3]

## Testing
- Test files live in [location]
- Run tests with: [command]
- [any important testing conventions]

## Key Constraints
- [critical constraint, e.g., "Never commit .env files"]
- [critical constraint, e.g., "Always run lint before committing"]
- [critical constraint, e.g., "Database migrations require manual review"]

## Common Tasks
### [Task Name]
[How to do it]
```

## Quality Checklist

Before finalizing, verify each file:

- [ ] No generic platitudes ("write good code", "be helpful")
- [ ] All stack information is accurate and specific
- [ ] File paths reference actual directories in the project
- [ ] Code examples match actual patterns in the codebase
- [ ] Test commands actually work
- [ ] Constraints reflect real project requirements

## Output Contract

- Create `.github/copilot-instructions.md` (always)
- Create `AGENTS.md` (always)  
- Create `.cursorrules` (if Cursor files requested or project uses Cursor)
- Create `CLAUDE.md` (if Claude-specific context requested)
- Each file must be specific to this codebase — no generic templates
- Reference actual files, directories, and patterns from the project
