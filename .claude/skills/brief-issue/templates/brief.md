## Task: ${title}

This is a brief for the issue-working agent. It is self-contained: you need only this brief and the repository at its base commit. Do this one task only.

### Base
- Branch from `${remote}/${base}` at **${base_commit}** or later, as `${lane}`. The `work-issue` skill's `start` step does this.
${setup}
### The task
[[FILL: what to build and why. Say where it is: the plan file and its line numbers at the base commit, and the task's record file if the plan is split, or the files involved. Say where its records go: the plan's Risks, or its record file. Paste every ruling and record the task needs, condensed: the worker cannot read the orchestrator's files. End with the acceptance criteria and their commands.]]

### Gate
- Run `${gate_command}`; its log must end `exit 0`. The `work-issue` skill's `gate run` stamps the log with the tree it ran on.
- [[FILL: task-specific checks with their commands (tests by name, measurements), or delete this line]]

### Rules
${rules}
### Report: comment on this issue when done
${report}
- [[FILL: task-specific report items (figures, timings with the machine and load), or delete this line]]

Post it with the `work-issue` skill's `report` step, which moves the issue from `${claimed}` to `${done}`. A question that blocks the task goes in a blocked report, with your lean (`${blocked}`); a question that doesn't block goes in its `comment` step, and the work goes on.

${marker}
