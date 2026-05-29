#!/usr/bin/env node

/**
 * validate-registry.js
 * RNA Method Registry Validator
 *
 * Runs 7 health checks against receptors.json and agent-context.json.
 *
 * Usage:
 *   node tools/validate-registry.js                                    — scan, report
 *   node tools/validate-registry.js --fix                              — scan + show auto-repair suggestions
 *   node tools/validate-registry.js --json                             — machine-readable JSON output
 *   node tools/validate-registry.js --root /path/to/project           — custom project root
 *   node tools/validate-registry.js --receptors path/receptors.json   — custom receptors path
 *   node tools/validate-registry.js --context path/agent-context.json — custom context path
 */

const fs = require('fs')
const path = require('path')

// ─── CLI Arguments ─────────────────────────────────────────────────────────

const args = process.argv.slice(2)
const FIX_MODE = args.includes('--fix')
const JSON_MODE = args.includes('--json')

function getArg(flag) {
  const idx = args.indexOf(flag)
  return idx !== -1 ? args[idx + 1] : null
}

const ROOT = path.resolve(getArg('--root') || process.cwd())
const RECEPTORS_PATH = path.resolve(
  getArg('--receptors') || path.join(ROOT, '_memory', 'rna-method', 'receptors.json')
)
const CONTEXT_PATH = path.resolve(
  getArg('--context') || path.join(ROOT, '_memory', 'rna-method', 'agent-context.json')
)

// ─── Pure Helpers (module-level) ───────────────────────────────────────────

function daysSince(isoDate) {
  return Math.floor((Date.now() - new Date(isoDate).getTime()) / 86400000)
}

// ─── run() ─────────────────────────────────────────────────────────────────
//
// Programmatic entry point. Accepts option overrides; always returns results.
// Prints and exits only when called from the CLI (require.main === module).
//
// @param {object} opts
//   opts.root       — project root (default: module-level ROOT)
//   opts.receptors  — path to receptors.json (default: module-level RECEPTORS_PATH)
//   opts.context    — path to agent-context.json (default: module-level CONTEXT_PATH)
//   opts.fix        — boolean (default: FIX_MODE)
//   opts.json       — boolean (default: JSON_MODE)
//   opts.silent     — boolean; when true, suppress all console output (default: false)
// @returns {{ passed, failed, warnings, suggestions }}

function run(opts = {}) {
  const root          = opts.root      ? path.resolve(opts.root)      : ROOT
  const receptorsPath = opts.receptors ? path.resolve(opts.receptors) : RECEPTORS_PATH
  const contextPath   = opts.context   ? path.resolve(opts.context)   : CONTEXT_PATH
  const fixMode       = opts.fix  !== undefined ? opts.fix  : FIX_MODE
  const jsonMode      = opts.json !== undefined ? opts.json : JSON_MODE
  const silent        = !!opts.silent

  const results = { passed: [], failed: [], warnings: [], suggestions: [] }
  let receptors, context

  // ── Closure helpers ──────────────────────────────────────────────────────

  function pass(check, detail) {
    results.passed.push({ check, detail })
  }

  function fail(check, detail, suggestion) {
    results.failed.push({ check, detail })
    if (suggestion) results.suggestions.push({ check, suggestion })
  }

  function warn(check, detail, suggestion) {
    results.warnings.push({ check, detail })
    if (suggestion) results.suggestions.push({ check, suggestion })
  }

  function fileExists(relPath) {
    return fs.existsSync(path.join(root, relPath))
  }

  // ── Load Files ────────────────────────────────────────────────────────────

  // ── Load Files ────────────────────────────────────────────────────────────

  try {
    receptors = JSON.parse(fs.readFileSync(receptorsPath, 'utf8'))
  } catch (e) {
    if (!silent && !jsonMode) {
      console.error(`✗ FATAL: Cannot read receptors.json at ${receptorsPath}`)
      console.error(`  ${e.message}`)
      console.error(`  Tip: Run with --receptors <path> to specify a custom location.`)
    }
    return { ...results, fatal: `Cannot read receptors.json: ${e.message}` }
  }

  try {
    context = JSON.parse(fs.readFileSync(contextPath, 'utf8'))
  } catch (e) {
    if (!silent && !jsonMode) {
      console.warn(`⚠ agent-context.json not found at ${contextPath}`)
      console.warn(`  Checkpoint checks will be skipped. Run with --context <path> to specify a location.`)
    }
    context = { checkpoints: [] }
  }

  // ─── Check 1: All agent .agent.md files exist ──────────────────────────

  const CHECK1 = 'agent-files-exist'
  let allAgentsExist = true
  for (const agent of receptors.agents) {
    if (!fileExists(agent.file)) {
      fail(
        CHECK1,
        `Agent "${agent.id}" file missing: ${agent.file}`,
        `Create the agent file at ${path.join(root, agent.file)}`
      )
      allAgentsExist = false
    }
  }
  if (allAgentsExist) pass(CHECK1, `All ${receptors.agents.length} agent files found`)

  // ─── Check 2: All skill canonical files exist ───────────────────────────

  const CHECK2 = 'skill-files-exist'
  let allSkillsExist = true
  for (const skill of (receptors.skills || [])) {
    if (!fileExists(skill.canonicalFile)) {
      fail(
        CHECK2,
        `Skill "${skill.id}" canonical file missing: ${skill.canonicalFile}`,
        `Create the skill file at ${path.join(root, skill.canonicalFile)}`
      )
      allSkillsExist = false
    }
    if (skill.file && !fileExists(skill.file)) {
      warn(
        CHECK2,
        `Skill "${skill.id}" platform wrapper missing: ${skill.file}`,
        `Create the skill wrapper at ${path.join(root, skill.file)}`
      )
    }
  }
  if (allSkillsExist) {
    const skillCount = (receptors.skills || []).length
    pass(CHECK2, `All ${skillCount} skill canonical files found`)
  }

  // ─── Check 3: All rule instruction files exist ──────────────────────────

  const CHECK3 = 'rule-files-exist'
  let allRulesExist = true
  for (const rule of (receptors.rules || [])) {
    if (!fileExists(rule.file)) {
      fail(
        CHECK3,
        `Rule "${rule.id}" file missing: ${rule.file}`,
        `Create the rule file at ${path.join(root, rule.file)}`
      )
      allRulesExist = false
    }
  }
  if (allRulesExist) pass(CHECK3, `All ${(receptors.rules || []).length} rule files found`)

  // ─── Check 4: All hook targets are valid ────────────────────────────────

  const CHECK4 = 'hook-targets-valid'
  let allHooksValid = true
  for (const hook of (receptors.hooks || [])) {
    if (hook.type === 'script' && hook.scriptPath) {
      if (!fileExists(hook.scriptPath)) {
        fail(
          CHECK4,
          `Hook "${hook.id}" script missing: ${hook.scriptPath}`,
          `Create the script at ${path.join(root, hook.scriptPath)}`
        )
        allHooksValid = false
      }
    } else if (hook.type === 'instruction') {
      const rule = (receptors.rules || []).find(r => r.id === hook.activates)
      if (!rule) {
        fail(
          CHECK4,
          `Hook "${hook.id}" references unknown rule ID: "${hook.activates}"`,
          `Add rule "${hook.activates}" to the rules[] array in receptors.json`
        )
        allHooksValid = false
      }
    }
  }
  if (allHooksValid) pass(CHECK4, `All ${(receptors.hooks || []).length} hook targets valid`)

  // ─── Check 5: Agent IDs are unique and self-consistent ──────────────────

  const CHECK5 = 'agent-ids-unique'
  const agentIds = (receptors.agents || []).map(a => a.id)
  const duplicates = agentIds.filter((id, idx) => agentIds.indexOf(id) !== idx)
  if (duplicates.length > 0) {
    fail(
      CHECK5,
      `Duplicate agent IDs found: ${duplicates.join(', ')}`,
      `Each agent must have a unique "id" field in receptors.json agents[]`
    )
  } else {
    pass(CHECK5, `All ${agentIds.length} agent IDs are unique`)
  }

  // ─── Check 6: No orphaned checkpoint pointers ───────────────────────────

  const CHECK6 = 'no-orphaned-checkpoints'
  const checkpoints = context.checkpoints || []
  let allCheckpointsClean = true
  for (const cp of checkpoints) {
    if (!fileExists(cp.path)) {
      fail(
        CHECK6,
        `Orphaned checkpoint: taskSlug="${cp.taskSlug}" → path="${cp.path}" (file missing)`,
        fixMode
          ? `Remove { taskId: "${cp.taskId}" } from agent-context.json checkpoints[]`
          : `Run with --fix to see auto-repair suggestions`
      )
      allCheckpointsClean = false
    }
  }
  if (allCheckpointsClean) {
    pass(CHECK6, `All ${checkpoints.length} checkpoint pointer(s) resolve to existing files`)
  }

  // ─── Check 7: No stale checkpoints (> 7 days old) ───────────────────────

  const CHECK7 = 'no-stale-checkpoints'
  let hasStale = false
  for (const cp of checkpoints) {
    if (!cp.createdAt) continue
    const age = daysSince(cp.createdAt)
    if (age > 7) {
      warn(
        CHECK7,
        `Stale checkpoint: taskSlug="${cp.taskSlug}" is ${age} days old`,
        `If the task is complete, delete ${cp.path} and remove its entry from agent-context.json checkpoints[]`
      )
      hasStale = true
    }
  }
  if (!hasStale) pass(CHECK7, `No stale checkpoints found`)

  return results
}

// ─── CLI Entry ──────────────────────────────────────────────────────────────

if (require.main === module) {
  const r = run()

  if (r.fatal) {
    console.error(`✗ FATAL: ${r.fatal}`)
    process.exit(1)
  }

  if (JSON_MODE) {
    console.log(JSON.stringify({ root: ROOT, receptors: RECEPTORS_PATH, context: CONTEXT_PATH, ...r }, null, 2))
    process.exit(r.failed.length > 0 ? 1 : 0)
  }

  const GREEN  = '\x1b[32m'
  const RED    = '\x1b[31m'
  const YELLOW = '\x1b[33m'
  const BLUE   = '\x1b[34m'
  const RESET  = '\x1b[0m'

  console.log(`\n${BLUE}═══ RNA Method Registry Validator ═══${RESET}`)
  console.log(`${BLUE}    Root: ${ROOT}${RESET}\n`)

  for (const p of r.passed)   console.log(`${GREEN}✓${RESET} [${p.check}] ${p.detail}`)
  for (const w of r.warnings) console.log(`${YELLOW}⚠${RESET} [${w.check}] ${w.detail}`)
  for (const f of r.failed)   console.log(`${RED}✗${RESET} [${f.check}] ${f.detail}`)

  console.log(`\n─── Summary ──────────────────────────────────────────`)
  console.log(`  Passed:   ${r.passed.length}`)
  console.log(`  Warnings: ${r.warnings.length}`)
  console.log(`  Failed:   ${r.failed.length}`)

  if (r.suggestions.length > 0) {
    console.log(`\n─── Suggestions ${FIX_MODE ? '(auto-repair mode)' : '(run --fix for repairs)'} ───`)
    for (const s of r.suggestions) console.log(`  [${s.check}] ${s.suggestion}`)
  }

  if (r.failed.length === 0 && r.warnings.length === 0) {
    console.log(`\n${GREEN}All checks passed. Registry is healthy.${RESET}\n`)
  } else if (r.failed.length === 0) {
    console.log(`\n${YELLOW}Passed with warnings. Registry is functional but has gaps.${RESET}\n`)
  } else {
    console.log(`\n${RED}Registry has failures. Fix the issues above before proceeding.${RESET}\n`)
  }

  process.exit(r.failed.length > 0 ? 1 : 0)
}

module.exports = { run }

