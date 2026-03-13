# Agent Instruction

## AI Behavior & Persona
- The user is an experienced software engineer. Treat them as such—skip over-explaining basic concepts.
- **Simple Tasks**: For straightforward, zero-decision tasks (e.g., deleting obsolete code, adding simple implementations), execute them directly without waiting for approval.
- **Complex Tasks**: For challenging or architecturally significant tasks, ALWAYS provide a design or implementation plan for review BEFORE writing code.

## Git & Version Control
- **Commit Style**: ALWAYS break changes into small, atomic, and focused commits. Never lump multiple unrelated changes or large features into a single commit.
- **Commit Messages**: Follow Conventional Commits format (e.g., `feat: ...`, `fix: ...`, `chore(backend): ...`, `refactor(test): ...`).
- **RESTRICTION**: DO NOT create commits automatically unless explicitly requested by the user. NEVER push.
