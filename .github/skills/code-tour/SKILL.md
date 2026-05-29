---
name: code-tour
description: 'Generate step-by-step interactive CodeTour files for VS Code that guide developers through unfamiliar codebases. Use when a user asks for a code tour, walkthrough, or guided exploration of a codebase or feature.'
---

# Code Tour Generation

Generate step-by-step interactive CodeTour files for VS Code that guide developers through unfamiliar codebases.

## Overview

[CodeTour](https://marketplace.visualstudio.com/items?itemName=vsls-contrib.codetour) is a VS Code extension that lets developers create interactive, annotated walkthroughs of a codebase. Tours are stored as JSON files in `.tours/` and can be shared and version-controlled.

## When to Use

Use this skill when a user asks to:
- Create a code tour or walkthrough
- Generate guided exploration of a codebase
- Onboard new developers to a project
- Document how a feature works end-to-end

## Tour File Format

Tours are stored in `.tours/<tour-name>.tour` as JSON:

```json
{
  "$schema": "https://aka.ms/codetour-schema",
  "title": "Tour Title",
  "description": "Optional description",
  "steps": [
    {
      "file": "relative/path/to/file.ts",
      "line": 42,
      "title": "Step Title",
      "description": "Markdown description of what's happening here"
    }
  ]
}
```

## Step Types

### File + Line Reference
```json
{
  "file": "src/index.ts",
  "line": 10,
  "title": "Entry Point",
  "description": "The application starts here..."
}
```

### Directory Reference
```json
{
  "directory": "src/components",
  "title": "Components Directory",
  "description": "All React components live here..."
}
```

### External URL Reference
```json
{
  "uri": "https://example.com/docs",
  "title": "Documentation",
  "description": "Refer to the official docs..."
}
```

### Shell Command Step
```json
{
  "title": "Install Dependencies",
  "description": "Run this command to install dependencies",
  "commands": ["npm install"]
}
```

## Workflow

1. **Explore the codebase** — understand the project structure, entry points, and key flows
2. **Identify the narrative** — what story does this tour tell? (e.g., "How a request flows through the system")
3. **Select 5–15 key stops** — focus on non-obvious, high-value points
4. **Write descriptive annotations** — each step should explain *why*, not just *what*
5. **Order logically** — follow execution order, data flow, or conceptual progression
6. **Create the `.tour` file** — save to `.tours/<descriptive-name>.tour`

## Writing Good Step Descriptions

- Explain the *purpose* and *why*, not just what the code does
- Use Markdown for formatting (headers, bold, code blocks, lists)
- Keep descriptions concise but informative (2–5 sentences)
- Reference other files or concepts when helpful
- Highlight gotchas, non-obvious patterns, or architectural decisions

### Example Good Description
```
This is the main router configuration. Notice how routes are **lazily loaded** using dynamic imports — this keeps the initial bundle small. The `AuthGuard` wrapper on protected routes ensures unauthenticated users are redirected to `/login`.
```

### Example Poor Description
```
This is the router file. It has routes defined in it.
```

## Tour Types to Generate

### Architecture Tour
Walk through the high-level structure: entry point → configuration → core modules → utilities

### Feature Tour
Trace a specific feature end-to-end: UI component → event handler → API call → backend route → database query

### Onboarding Tour
Guide new developers: project structure → setup files → key conventions → first contribution path

### Bug Investigation Tour
Show where a bug manifests, root cause, and the fix

## Output Contract

- Create `.tours/<tour-name>.tour` with valid JSON matching the CodeTour schema
- Include 5–15 steps (adjust based on codebase complexity)
- Each step must have a `title` and `description`
- File paths must be relative to the workspace root
- Line numbers should point to the most relevant line (e.g., function definition, key logic)
- Tour title should be descriptive (e.g., "Authentication Flow", "New Developer Onboarding")

## Example Tours

### Simple Tour Structure
```json
{
  "$schema": "https://aka.ms/codetour-schema",
  "title": "Request Lifecycle",
  "description": "Follow an HTTP request from entry to response",
  "steps": [
    {
      "file": "src/server.ts",
      "line": 1,
      "title": "Server Entry Point",
      "description": "The Express server is initialized here. Note how middleware is applied in order — authentication runs before route handlers."
    },
    {
      "file": "src/routes/index.ts",
      "line": 1,
      "title": "Route Registration",
      "description": "All routes are registered here. The `/api` prefix is applied globally, so individual route files don't need to repeat it."
    },
    {
      "file": "src/middleware/auth.ts",
      "line": 15,
      "title": "Authentication Middleware",
      "description": "JWT tokens are validated here. If invalid, a 401 is returned immediately. The `req.user` object is populated for downstream handlers."
    }
  ]
}
```
