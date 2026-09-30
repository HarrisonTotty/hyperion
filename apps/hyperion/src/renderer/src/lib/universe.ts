import {
  RequestChannel,
  type RequestClient,
  type RequestOf,
  type ResponseFor,
  type SeedHex,
  type UniverseIdHex,
  type UniverseInfo,
} from "@hyperion/protocol";
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";

import { useServerLink } from "./serverLink";
import {
  followRequest,
  PENDING,
  REQUEST_TIMEOUT_MS,
  type RequestState,
  type SettledRequestState,
  useServerRequest,
} from "./useServerRequest";

/** The kinds of the commands a universe session sends: they change what the server holds. */
type CommandKind = "create_universe" | "open_universe";

/** Where the last create or open command stands. */
export type UniverseCommandState = RequestState<"create_universe"> | RequestState<"open_universe">;

/**
 * The universes on the server and the one this console has open.
 *
 * @remarks
 * Requests are stateless and each names its universe (plan 04, design note 6), so "open" is the
 * client's choice of universe: nothing is re-opened after a reconnect. The actions are plain
 * functions, not methods, so that a display may take them out of the session.
 */
export interface UniverseSession {
  /** The universe the displays show, or `null` before one is created or opened. */
  readonly open: UniverseInfo | null;
  /** The server's universes, requested once the link is up and after every create or reconnect. */
  readonly list: RequestState<"list_universes">;
  /** The last create or open command. */
  readonly command: UniverseCommandState;
  /**
   * Whether a create was cut off by a lost link, so that the server may or may not have made the
   * universe; the refreshed list says which. Cleared by the next create, by a universe opened from
   * the list, or by {@link UniverseSession.dismissUnconfirmed}.
   */
  readonly createUnconfirmed: boolean;
  /**
   * Creates a universe and opens it once the server has made it.
   *
   * @param name - The operator's name for it, already checked and trimmed.
   * @param seed - The seed in wire form, or `null` for one the server draws.
   */
  readonly create: (name: string, seed: SeedHex | null) => void;
  /** Opens a listed universe once the server has checked and loaded it. */
  readonly openUniverse: (universe: UniverseIdHex) => void;
  /** Requests the universe list again. */
  readonly refresh: () => void;
  /** Clears {@link UniverseSession.createUnconfirmed}, once the operator has seen it. */
  readonly dismissUnconfirmed: () => void;
}

/**
 * Hands the universe session to every display.
 *
 * @remarks
 * `UniverseProvider` supplies it, below the server link. `null` outside a provider, which
 * {@link useUniverse} reports.
 */
export const UniverseContext = createContext<UniverseSession | null>(null);

/**
 * The universe session from the nearest {@link UniverseContext}.
 *
 * @throws Error when no provider is above the caller, which is a wiring bug.
 */
export function useUniverse(): UniverseSession {
  const session = useContext(UniverseContext);
  if (session === null) {
    throw new Error("useUniverse needs a UniverseContext provider above it, as App supplies");
  }
  return session;
}

const LIST_UNIVERSES: RequestOf<"list_universes"> = { kind: "list_universes" };

const NO_COMMAND = { kind: "idle" } as const;

/** Sends the session's commands on a channel of their own, one at a time. */
class CommandSender {
  readonly #channel: RequestChannel;
  #stopFollowing: (() => void) | null = null;

  constructor(requests: RequestClient) {
    this.#channel = new RequestChannel(requests);
  }

  /** Sends a command, superseding the one before it, and reports its settled state. */
  send<K extends CommandKind>(
    body: RequestOf<K>,
    onSettled: (state: SettledRequestState<K>) => void,
  ): void {
    this.stop();
    this.#stopFollowing = followRequest(
      this.#channel.request(body),
      REQUEST_TIMEOUT_MS,
      (state) => {
        this.#stopFollowing = null;
        onSettled(state);
      },
    );
  }

  /** Stops following the command in flight and cancels it. */
  stop(): void {
    this.#stopFollowing?.();
    this.#stopFollowing = null;
  }
}

function universeInfo(response: ResponseFor<CommandKind>): UniverseInfo {
  const { id, name, seed, generator_version, status } = response;
  return { id, name, seed, generator_version, status };
}

/**
 * Runs the universe session that `UniverseProvider` hands to the displays.
 *
 * @remarks
 * The list goes through {@link useServerRequest}, sent again on every refresh, after every create
 * that succeeds or times out, and each time the link comes up. Create and open are commands: they go
 * through a channel of their own, show `pending` and then the server's answer, and are never sent
 * again by the client (plan 05, design note D4). A create's answer is the new universe, so it is
 * opened without a second request; an open's answer means the server has checked and warmed it. A
 * create cut off by a lost link is reported as unconfirmed once the link returns, since the server
 * may have made the universe; the list refreshed on reconnect shows whether it did, and a universe
 * opened from it clears the report.
 * The session is memoised, so its consumers re-render only when it changes.
 */
export function useUniverseSession(): UniverseSession {
  const { status, requests } = useServerLink();
  const connected = status === "connected";

  const [open, setOpen] = useState<UniverseInfo | null>(null);
  const [command, setCommand] = useState<UniverseCommandState>(NO_COMMAND);
  // Which command `command` is the state of, so that a create lost with the link can be told apart.
  const [commandKind, setCommandKind] = useState<CommandKind | null>(null);
  const [createUnconfirmed, setCreateUnconfirmed] = useState(false);
  const [sender] = useState(() => new CommandSender(requests));

  const [linkUps, setLinkUps] = useState({ connected, count: connected ? 1 : 0 });
  let timesLinkCameUp = linkUps.count;
  let shownCommand = command;
  let shownUnconfirmed = createUnconfirmed;
  if (linkUps.connected !== connected) {
    timesLinkCameUp += connected ? 1 : 0;
    setLinkUps({ connected, count: timesLinkCameUp });
    // A command lost with the link has no known result; once the link is back, the refreshed list
    // shows whether it took effect, and the lost link is no longer news. A create may have made a
    // universe the operator did not see opened, so it is reported as unconfirmed until dismissed.
    if (connected && command.kind === "link_down") {
      setCommand(NO_COMMAND);
      shownCommand = NO_COMMAND;
      if (commandKind === "create_universe") {
        setCreateUnconfirmed(true);
        shownUnconfirmed = true;
      }
    }
  }
  const [refreshes, setRefreshes] = useState(0);
  const list = useServerRequest(LIST_UNIVERSES, REQUEST_TIMEOUT_MS, timesLinkCameUp + refreshes);
  useEffect(
    () => () => {
      sender.stop();
    },
    [sender],
  );

  const refresh = useCallback(() => {
    setRefreshes((count) => count + 1);
  }, []);

  const dismissUnconfirmed = useCallback(() => {
    setCreateUnconfirmed(false);
  }, []);

  const create = useCallback(
    (name: string, seed: SeedHex | null) => {
      setCommand(PENDING);
      setCommandKind("create_universe");
      setCreateUnconfirmed(false);
      sender.send({ kind: "create_universe", name, seed }, (state) => {
        setCommand(state);
        if (state.kind === "ok") {
          setOpen(universeInfo(state.response));
        }
        // The server may have made the universe before the cancel of a timed-out create reached
        // it, so the list is refreshed then too, and shows whether it did.
        if (state.kind === "ok" || state.kind === "timed_out") {
          setRefreshes((count) => count + 1);
        }
      });
    },
    [sender],
  );

  const openUniverse = useCallback(
    (universe: UniverseIdHex) => {
      setCommand(PENDING);
      setCommandKind("open_universe");
      sender.send({ kind: "open_universe", universe }, (state) => {
        setCommand(state);
        if (state.kind === "ok") {
          setOpen(universeInfo(state.response));
          // Opening from the list is acting on it: an unconfirmed create has been checked.
          setCreateUnconfirmed(false);
        }
      });
    },
    [sender],
  );

  return useMemo(
    () => ({
      open,
      list,
      command: shownCommand,
      createUnconfirmed: shownUnconfirmed,
      create,
      openUniverse,
      refresh,
      dismissUnconfirmed,
    }),
    [open, list, shownCommand, shownUnconfirmed, create, openUniverse, refresh, dismissUnconfirmed],
  );
}
