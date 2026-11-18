/**
 * The page's next task (lane perf-long-tasks-3): work split off the task that asks for it, so that
 * a frame, or the page's input, runs between the two.
 *
 * @remarks
 * A message on a channel of its own, which runs once the tasks queued before it have, and which a
 * hidden page does not throttle as it throttles timers (the views check's windows are hidden), nor
 * a test's fake clock hold.
 */

/** Runs `task` in a task of its own, after those already queued. */
export function nextTask(task: () => void): void {
  const channel = new MessageChannel();
  channel.port1.addEventListener(
    "message",
    () => {
      channel.port1.close();
      task();
    },
    { once: true },
  );
  channel.port1.start();
  channel.port2.postMessage(null);
}

/** Settles in a task of its own, after those already queued ({@link nextTask}). */
export function untilNextTask(): Promise<void> {
  return new Promise((resolve) => {
    nextTask(resolve);
  });
}
