/**
 * The page's next task (lane perf-long-tasks-3): work split off the task that asks for it, so that
 * a frame, or the page's input, runs between the two.
 *
 * @remarks
 * A message on a channel of its own, which runs once the tasks queued before it have, and which a
 * hidden page does not throttle as it throttles timers (the views check's windows are hidden); or
 * a timer of no delay, whichever comes first, so that a test whose fake clock advances timers runs
 * it there too, though it gives the channel's message no turn. The work runs once.
 */

/** Runs `task` in a task of its own, after those already queued. */
export function nextTask(task: () => void): void {
  const channel = new MessageChannel();
  let ran = false;
  const run = (): void => {
    if (ran) {
      return;
    }
    ran = true;
    clearTimeout(timer);
    channel.port1.close();
    task();
  };
  const timer = setTimeout(run, 0);
  channel.port1.addEventListener("message", run, { once: true });
  channel.port1.start();
  channel.port2.postMessage(null);
}

/** Settles in a task of its own, after those already queued ({@link nextTask}). */
export function untilNextTask(): Promise<void> {
  return new Promise((resolve) => {
    nextTask(resolve);
  });
}
