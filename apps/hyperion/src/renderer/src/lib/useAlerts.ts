/**
 * A display's sensor detections as it holds them (plan 12, P12.T12): the alerts subscription, the
 * contacts and census it keeps, the observer it moves and the acknowledgements it sends.
 */
import {
  type AlertsNotification,
  type ClassCensusDto,
  type ContactDto,
  galacticPositionFromLy,
  type NotificationOf,
  type PendingSubscription,
  type RequestClient,
  type RequestFailure,
  type SensorDto,
  type StateOf,
  type Subscription,
  type UniverseIdHex,
  universeTimeFromYears,
} from "@hyperion/protocol";
import { useEffect, useState, useSyncExternalStore } from "react";

import { RECONNECT_DELAY_MS } from "./connection";
import { useServerLink } from "./serverLink";
import { followRequest, REQUEST_TIMEOUT_MS } from "./useServerRequest";

/**
 * The V band's in-band flux of a star of magnitude 0, W m⁻²: Bessell, Castelli and Plez's (1998,
 * A&A 333, 231, Table A2) zero point, 363.1 × 10⁻¹¹ erg cm⁻² s⁻¹ Å⁻¹ (3.631 × 10⁻¹¹ W m⁻² nm⁻¹),
 * through the server's top hat of Bessell's (2005, ARA&A 43, 293, Table 1) 840 Å, the band's flux
 * as the wire reads it (`AlertBandDto`).
 */
const V_ZERO_FLUX_W_M2 = 3.631e-11 * 84;

/**
 * The Crab nebula's flux between 2 and 10 keV, W m⁻²: 2.4 × 10⁻⁸ erg cm⁻² s⁻¹ (Kirsch et al. 2005,
 * Proc. SPIE 5898, 22), the X-ray astronomers' unit, the crab.
 */
const CRAB_FLUX_W_M2 = 2.4e-11;

/** The flux of a source of V magnitude `magnitude` in the V band, W m⁻². */
function vFluxWm2(magnitude: number): number {
  return V_ZERO_FLUX_W_M2 * 10 ** (-0.4 * magnitude);
}

/**
 * The sensor every alerts subscription names (plan 12's non-goals: a sensor is two flux limits per
 * band), a stand-in until the ship has a sensor model (lean, 2026-11-01).
 *
 * @remarks
 * Two bands, each a detection and a resolve limit a factor of 100 apart, five magnitudes:
 * - V detects to magnitude 17, a wide-field transient survey's depth (ASAS-SN's V ≲ 17, Shappee et
 *   al. 2014, ApJ 788, 48), 4.83 × 10⁻¹⁶ W m⁻², and resolves the host from magnitude 12,
 *   4.83 × 10⁻¹⁴ W m⁻² ({@link V_ZERO_FLUX_W_M2}).
 * - X-rays, 2–10 keV, detect from 10 mCrab, 2.4 × 10⁻¹³ W m⁻², about an all-sky monitor's, and
 *   resolve from 1 Crab, 2.4 × 10⁻¹¹ W m⁻² ({@link CRAB_FLUX_W_M2}).
 *
 * The limits are the lean's choice, not a model of any instrument: the sensor consoles replace them.
 * Every class is asked for, and the server's own host limit is taken.
 */
export const STAND_IN_SENSOR: SensorDto = {
  bands: [
    { band: "v", detect_w_m2: vFluxWm2(17), resolve_w_m2: vFluxWm2(12) },
    { band: "x_ray", detect_w_m2: CRAB_FLUX_W_M2 * 0.01, resolve_w_m2: CRAB_FLUX_W_M2 },
  ],
};

/** Where an alerts subscription's observer stands, and when: the display's observer and time. */
export interface AlertObserver {
  /** Its position in the `GALACTIC` frame, ly. */
  readonly positionLy: readonly [number, number, number];
  /** Its time, universe time in Julian years. */
  readonly timeYr: number;
}

/**
 * Why a display's detections could not be opened: the server's refusal, or `unusable` for a state
 * or a run of pushes the client cannot read.
 */
export type AlertsRefusalCode =
  Exclude<RequestFailure["code"], "aborted" | "superseded" | "link_lost"> | "unusable";

/**
 * Where a display's alerts subscription stands.
 *
 * @remarks
 * `no_observer` is a universe open with no observer chosen yet, which a subscription needs.
 * `stale` keeps the contacts on show, because the link is down (`link_down`), every value then
 * stale, or because the subscription is being opened again after a gap in the numbering, a push
 * the client could not use or the server's ending it (`resubscribing`). `link_down` is the link
 * being down before anything arrived. `rejected` and `timed_out` stay until the link comes back or
 * `retry`.
 */
export type AlertsStatus =
  | { readonly kind: "idle" }
  | { readonly kind: "no_observer" }
  | { readonly kind: "pending" }
  | { readonly kind: "live" }
  | { readonly kind: "stale"; readonly reason: "link_down" | "resubscribing" }
  | { readonly kind: "link_down" }
  | { readonly kind: "rejected"; readonly code: AlertsRefusalCode; readonly reason: string }
  | { readonly kind: "timed_out" };

/**
 * An acknowledgement sent and not yet confirmed, closed-loop (the guide's commanding): `pending`
 * until the server answers, then gone once it accepts, or its refusal in the server's words, or
 * unanswered in {@link REQUEST_TIMEOUT_MS}.
 */
export type Acknowledgement =
  | { readonly kind: "pending" }
  | { readonly kind: "rejected"; readonly reason: string }
  | { readonly kind: "timed_out" };

/** A move of the observer the server did not accept: refused with its reason, or timed out. */
export type ObserverFault =
  { readonly kind: "refused"; readonly reason: string } | { readonly kind: "timed_out" };

/** What a display's detections hold at one moment, as a render reads them. */
export interface AlertsSnapshot {
  readonly status: AlertsStatus;
  /**
   * Every contact the subscription holds that has not cleared, each as it last stood, in contact
   * order.
   */
  readonly contacts: ReadonlyArray<ContactDto>;
  /** Each class's census, in class order: empty until the server has scanned. */
  readonly census: ReadonlyArray<ClassCensusDto>;
  /** The acknowledgements not yet confirmed, by contact. */
  readonly acknowledgements: ReadonlyMap<number, Acknowledgement>;
  /** What became of the latest move of the observer, when it was not accepted; `null` else. */
  readonly observerFault: ObserverFault | null;
}

/** A display's detections and what it can do with them. */
export interface AlertsView extends AlertsSnapshot {
  /**
   * Acknowledges a contact: sends `alerts_acknowledge` and holds it {@link Acknowledgement}
   * `pending` until the server answers. Does nothing for a contact acknowledged already or being
   * acknowledged, or with no subscription open. Stable for the life of the subscription.
   */
  readonly acknowledge: (contact: number) => void;
  /** Opens the subscription again after it was refused or went unanswered; nothing otherwise. */
  readonly retry: () => void;
}

/** Resubscriptions in a row, with no push applied between them, after which the store gives up. */
const MAX_RESUBSCRIPTIONS = 3;

const NO_ACKNOWLEDGEMENTS: ReadonlyMap<number, Acknowledgement> = new Map();

const IDLE: AlertsSnapshot = {
  status: { kind: "idle" },
  contacts: [],
  census: [],
  acknowledgements: NO_ACKNOWLEDGEMENTS,
  observerFault: null,
};

/** Whether two observers are the same place at the same time. */
function sameObserver(a: AlertObserver | null, b: AlertObserver | null): boolean {
  return (
    a === b ||
    (a !== null &&
      b !== null &&
      a.timeYr === b.timeYr &&
      a.positionLy[0] === b.positionLy[0] &&
      a.positionLy[1] === b.positionLy[1] &&
      a.positionLy[2] === b.positionLy[2])
  );
}

/** Whether a value read from the wire is a finite number. */
function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

/**
 * Whether a contact as read from the wire can be shown: its flux finite and not negative, its
 * bearing's angles finite, and for a resolved one its light age and stated error too, so that no
 * `NaN` reaches a formatter.
 */
export function contactUsable(contact: ContactDto): boolean {
  const { bearing } = contact;
  const bearingUsable =
    bearing === null ||
    (isFiniteNumber(bearing.azimuth_deg) && isFiniteNumber(bearing.elevation_deg));
  const base =
    Number.isInteger(contact.contact) &&
    isFiniteNumber(contact.flux_w_m2) &&
    contact.flux_w_m2 >= 0 &&
    isFiniteNumber(contact.first_seen.seconds) &&
    isFiniteNumber(contact.last_seen.seconds) &&
    (contact.cleared === undefined || isFiniteNumber(contact.cleared.seconds)) &&
    bearingUsable;
  let usable: boolean;
  switch (contact.level) {
    case "bearing":
      usable = base;
      break;
    case "resolved":
      usable =
        base &&
        isFiniteNumber(contact.light_age_yr) &&
        contact.light_age_yr >= 0 &&
        isFiniteNumber(contact.curvature_error_ly);
      break;
  }
  return usable;
}

/**
 * Whether a contact is lit: its light has not fallen under the sensor's detection limit since its
 * latest sighting. A cleared contact's condition has ended, and the list drops it (Design note 11;
 * plan 12, P12.T8.c).
 */
export function contactLit(contact: ContactDto): boolean {
  return contact.cleared === undefined;
}

/** The contacts of a map, in contact order. */
function inContactOrder(contacts: ReadonlyMap<number, ContactDto>): ContactDto[] {
  return [...contacts.values()].toSorted((a, b) => a.contact - b.contact);
}

/**
 * One alerts subscription and what it holds, outside React: `useAlerts` reads it through
 * `useSyncExternalStore` and drives it from effects.
 */
class AlertsStore {
  readonly #listeners = new Set<() => void>();
  #snapshot: AlertsSnapshot;
  /** The contacts held, by number. */
  #contacts = new Map<number, ContactDto>();
  /** The observer the display has now, which the server is to be told. */
  #wanted: AlertObserver | null = null;
  /** The observer the server was last told, by the subscription or a move it accepted. */
  #told: AlertObserver | null = null;
  /** Whether the display wants the subscription open: the link is up and an effect holds it. */
  #wantOpen = false;
  /** The last notification's `sequence` applied, 0 before the first. */
  #sequence = 0;
  /** Resubscriptions since a push was last applied. */
  #resubscriptions = 0;
  /** Bumped by every opening and closing, so that a callback of an earlier one does nothing. */
  #generation = 0;
  #pending: PendingSubscription<"alerts"> | null = null;
  #subscription: Subscription<"alerts"> | null = null;
  #stopOpening: (() => void) | null = null;
  /** Stops following the move in flight; `null` with none in flight. */
  #stopMove: (() => void) | null = null;
  /** Stops following each acknowledgement in flight. */
  readonly #stopAcknowledgements = new Map<number, () => void>();
  /** The wait before opening again after the server ended the subscription. */
  #retry: ReturnType<typeof setTimeout> | undefined;

  constructor(
    readonly requests: RequestClient,
    readonly universe: UniverseIdHex | null,
  ) {
    this.#snapshot = IDLE;
  }

  /** Whether this store serves these inputs, or `useAlerts` must replace it. */
  serves(requests: RequestClient, universe: UniverseIdHex | null): boolean {
    return this.requests === requests && this.universe === universe;
  }

  /** Adds a listener for every change of the snapshot, as `useSyncExternalStore` asks. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** The current snapshot, the same object until it changes. */
  readonly getSnapshot = (): AlertsSnapshot => this.#snapshot;

  /** Opens the subscription afresh, the link being up, as soon as there is an observer. */
  open(): void {
    this.#wantOpen = true;
    this.#resubscriptions = 0;
    this.#open();
  }

  /** Stops the subscription; what it held stays for the link's return. */
  close(): void {
    this.#wantOpen = false;
    this.#stop();
  }

  /** Marks what is held as cut off by the link, the subscription having ended with it. */
  linkDown(): void {
    this.#wantOpen = false;
    this.#stop();
    this.#goStale("link_down");
  }

  /**
   * Takes the display's observer: moves the open subscription's observer with `alerts_observer`,
   * one move in flight at a time and the latest sent once it settles, or opens the subscription
   * once there is one. With none, the subscription ends and nothing is held.
   */
  setObserver(observer: AlertObserver | null): void {
    if (sameObserver(observer, this.#wanted)) {
      return;
    }
    this.#wanted = observer;
    if (observer === null) {
      this.#stop();
      this.#contacts = new Map();
      this.#sequence = 0;
      this.#set({
        ...IDLE,
        status: this.universe === null ? { kind: "idle" } : { kind: "no_observer" },
      });
      return;
    }
    if (this.#subscription !== null) {
      this.#move();
    } else if (this.#pending === null) {
      this.#open();
    }
  }

  /** See {@link AlertsView.retry}. */
  readonly retry = (): void => {
    const status = this.#snapshot.status.kind;
    if (status !== "rejected" && status !== "timed_out") {
      return;
    }
    this.#resubscriptions = 0;
    this.#set({ status: { kind: "pending" } });
    this.#open();
  };

  /** See {@link AlertsView.acknowledge}. */
  readonly acknowledge = (contact: number): void => {
    const subscription = this.#subscription;
    const held = this.#contacts.get(contact);
    if (
      subscription === null ||
      held === undefined ||
      held.acknowledged ||
      this.#snapshot.acknowledgements.get(contact)?.kind === "pending"
    ) {
      return;
    }
    this.#setAcknowledgement(contact, { kind: "pending" });
    const stop = followRequest(
      this.requests.request({ kind: "alerts_acknowledge", subscription: subscription.id, contact }),
      REQUEST_TIMEOUT_MS,
      (state) => {
        this.#stopAcknowledgements.delete(contact);
        switch (state.kind) {
          case "ok": {
            // The server's acceptance is the result: its push of the contact, which an
            // acknowledgement of a contact acknowledged already does not send, may follow.
            const now = this.#contacts.get(contact);
            if (now !== undefined && !now.acknowledged) {
              this.#contacts.set(contact, { ...now, acknowledged: true });
            }
            this.#setAcknowledgement(contact, null, true);
            break;
          }
          case "rejected":
            this.#setAcknowledgement(contact, { kind: "rejected", reason: state.reason });
            break;
          case "timed_out":
            this.#setAcknowledgement(contact, { kind: "timed_out" });
            break;
          case "link_down":
            this.#setAcknowledgement(contact, null);
            break;
        }
      },
    );
    this.#stopAcknowledgements.set(contact, stop);
  };

  /** Opens the subscription, if the display wants it and has an observer. */
  #open(): void {
    const universe = this.universe;
    const observer = this.#wanted;
    if (universe === null || !this.#wantOpen) {
      return;
    }
    if (observer === null) {
      this.#set({ status: { kind: "no_observer" } });
      return;
    }
    this.#stop();
    const generation = this.#generation;
    const pending = this.requests.subscribe(universe, {
      topic: "alerts",
      observer: galacticPositionFromLy(observer.positionLy),
      time: universeTimeFromYears(observer.timeYr),
      sensor: STAND_IN_SENSOR,
    });
    this.#pending = pending;
    if (this.#contacts.size === 0 && this.#snapshot.status.kind !== "stale") {
      this.#set({ status: { kind: "pending" } });
    }
    const timer = setTimeout(() => {
      if (generation === this.#generation) {
        this.#stop();
        this.#set({ status: { kind: "timed_out" } });
      }
    }, REQUEST_TIMEOUT_MS);
    this.#stopOpening = () => {
      clearTimeout(timer);
    };
    pending.outcome
      .then((outcome) => {
        if (generation !== this.#generation) {
          if (outcome.ok) {
            outcome.subscription.unsubscribe();
          }
          return undefined;
        }
        clearTimeout(timer);
        this.#pending = null;
        if (outcome.ok) {
          this.#adopt(outcome.subscription, observer, generation);
          return undefined;
        }
        const { code, message } = outcome.error;
        if (code === "link_lost") {
          this.#goStale("link_down");
        } else if (code !== "aborted" && code !== "superseded") {
          this.#set({ status: { kind: "rejected", code, reason: message } });
        }
        return undefined;
      })
      .catch((error: unknown) => {
        console.error("an alerts subscription's outcome failed to settle:", error);
      });
  }

  /** Takes an opened subscription's state, and follows its pushes and its end. */
  #adopt(subscription: Subscription<"alerts">, told: AlertObserver, generation: number): void {
    const state: StateOf<"alerts"> = subscription.state;
    if (!state.contacts.every(contactUsable)) {
      subscription.unsubscribe();
      this.#set({
        status: { kind: "rejected", code: "unusable", reason: "a contact's values are unreadable" },
      });
      return;
    }
    this.#subscription = subscription;
    this.#told = told;
    this.#sequence = 0;
    this.#contacts = new Map(
      state.contacts.filter(contactLit).map((contact) => [contact.contact, contact]),
    );
    this.#set({
      status: { kind: "live" },
      contacts: inContactOrder(this.#contacts),
      census: state.census,
      acknowledgements: NO_ACKNOWLEDGEMENTS,
      observerFault: null,
    });
    subscription.onNotification((notification) => {
      this.#apply(notification);
    });
    subscription.onEnd((end) => {
      if (generation !== this.#generation) {
        return;
      }
      switch (end.kind) {
        case "link_lost":
          this.#subscription = null;
          this.#goStale("link_down");
          break;
        case "ended":
          this.#subscription = null;
          this.#resubscribe(
            `the server ended the alerts (${end.error.code}: ${end.error.message})`,
            end.error.code,
            RECONNECT_DELAY_MS,
          );
          break;
        case "unsubscribed":
          break;
      }
    });
    if (!sameObserver(this.#wanted, told)) {
      this.#move();
    }
  }

  /**
   * Applies a push in `sequence` order: one numbered at or before the last applied is dropped, and
   * one after a gap, whose predecessors are lost, opens the subscription again, whose state holds
   * every contact.
   */
  #apply(notification: NotificationOf<"alerts">): void {
    const body: AlertsNotification = notification;
    if (body.sequence <= this.#sequence) {
      return;
    }
    if (body.sequence !== this.#sequence + 1) {
      this.#resubscribe(
        `alerts notification ${String(body.sequence)} arrived where ${String(this.#sequence + 1)} was due`,
        "unusable",
        0,
      );
      return;
    }
    if (!body.contacts.every(contactUsable)) {
      this.#resubscribe("an alerts notification's contact is unreadable", "unusable", 0);
      return;
    }
    this.#sequence = body.sequence;
    this.#resubscriptions = 0;
    let acknowledgements = this.#snapshot.acknowledgements;
    for (const contact of body.contacts) {
      const lit = contactLit(contact);
      if (lit) {
        this.#contacts.set(contact.contact, contact);
      } else {
        this.#contacts.delete(contact.contact);
      }
      if ((contact.acknowledged || !lit) && acknowledgements.has(contact.contact)) {
        const next = new Map(acknowledgements);
        next.delete(contact.contact);
        acknowledgements = next;
      }
    }
    this.#set({
      status: { kind: "live" },
      contacts: inContactOrder(this.#contacts),
      // A census that has not changed is sent empty.
      census: body.census.length > 0 ? body.census : this.#snapshot.census,
      acknowledgements,
    });
  }

  /** Sends the display's observer, one move in flight at a time. */
  #move(): void {
    const subscription = this.#subscription;
    const observer = this.#wanted;
    if (subscription === null || observer === null || this.#stopMove !== null) {
      return;
    }
    this.#stopMove = followRequest(
      this.requests.request({
        kind: "alerts_observer",
        subscription: subscription.id,
        observer: galacticPositionFromLy(observer.positionLy),
        time: universeTimeFromYears(observer.timeYr),
      }),
      REQUEST_TIMEOUT_MS,
      (state) => {
        this.#stopMove = null;
        switch (state.kind) {
          case "ok":
            this.#told = observer;
            this.#set({ observerFault: null });
            break;
          case "rejected":
            this.#set({ observerFault: { kind: "refused", reason: state.reason } });
            break;
          case "timed_out":
            this.#set({ observerFault: { kind: "timed_out" } });
            break;
          case "link_down":
            return;
        }
        // A move made while this one was in flight goes now; one refused is not sent again.
        if (state.kind === "ok" && !sameObserver(this.#wanted, this.#told)) {
          this.#move();
        }
      },
    );
  }

  /**
   * Opens the subscription again, after `delayMs`, once it could not go on. After
   * {@link MAX_RESUBSCRIPTIONS} in a row with no push applied between, it gives up as `rejected`.
   */
  #resubscribe(why: string, code: AlertsRefusalCode, delayMs: number): void {
    this.#stop();
    this.#resubscriptions += 1;
    if (this.#resubscriptions > MAX_RESUBSCRIPTIONS) {
      console.error(`${why}; giving up after ${String(MAX_RESUBSCRIPTIONS)} resubscriptions`);
      this.#set({ status: { kind: "rejected", code, reason: why } });
      return;
    }
    console.error(`${why}; resubscribing`);
    this.#goStale("resubscribing");
    if (delayMs <= 0) {
      this.#open();
      return;
    }
    this.#retry = setTimeout(() => {
      this.#retry = undefined;
      this.#open();
    }, delayMs);
  }

  /** Marks what is held as stale, or the link as down with nothing held. */
  #goStale(reason: "link_down" | "resubscribing"): void {
    const opened = this.#snapshot.status.kind === "live" || this.#snapshot.status.kind === "stale";
    if (!opened) {
      if (reason === "link_down" && this.universe !== null) {
        this.#set({ status: { kind: "link_down" } });
      }
      return;
    }
    this.#set({ status: { kind: "stale", reason }, acknowledgements: NO_ACKNOWLEDGEMENTS });
  }

  /** Ends every opening, subscription, move and acknowledgement in progress. */
  #stop(): void {
    this.#generation += 1;
    this.#stopOpening?.();
    this.#stopOpening = null;
    this.#pending?.cancel();
    this.#pending = null;
    this.#subscription?.unsubscribe();
    this.#subscription = null;
    this.#stopMove?.();
    this.#stopMove = null;
    for (const stop of this.#stopAcknowledgements.values()) {
      stop();
    }
    this.#stopAcknowledgements.clear();
    clearTimeout(this.#retry);
    this.#retry = undefined;
  }

  /**
   * Sets a contact's acknowledgement, or clears it with `null`; `refresh` takes the contacts again,
   * as an accepted acknowledgement has changed one.
   */
  #setAcknowledgement(contact: number, state: Acknowledgement | null, refresh = false): void {
    const next = new Map(this.#snapshot.acknowledgements);
    if (state === null) {
      next.delete(contact);
    } else {
      next.set(contact, state);
    }
    this.#set({
      acknowledgements: next,
      ...(refresh ? { contacts: inContactOrder(this.#contacts) } : {}),
    });
  }

  /** Changes the snapshot, notifying only for a real change. */
  #set(change: Partial<AlertsSnapshot>): void {
    const current = this.#snapshot;
    const next: AlertsSnapshot = { ...current, ...change };
    const same =
      JSON.stringify(next.status) === JSON.stringify(current.status) &&
      next.contacts === current.contacts &&
      next.census === current.census &&
      next.acknowledgements === current.acknowledgements &&
      JSON.stringify(next.observerFault) === JSON.stringify(current.observerFault);
    if (same) {
      return;
    }
    this.#snapshot = next;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

/**
 * Subscribes a display to the sensor detections of `universe` seen from its observer, and keeps
 * them: the contacts, the census per class and the acknowledgements in flight (plan 12, P12.T12).
 *
 * @remarks
 * Subscribes with the stand-in sensor ({@link STAND_IN_SENSOR}) once the link is up and the display
 * has an observer, and again when the link returns. A changed observer or time is sent with
 * `alerts_observer`, one move in flight at a time, the latest sent once it settles; one the server
 * refuses is reported as {@link AlertsSnapshot.observerFault}. Notifications are taken through
 * `@hyperion/protocol` in their `sequence` order: one numbered at or before the last applied is
 * dropped, and a gap, or a push the client cannot use, opens the subscription again, whose state
 * holds every contact the universe's Knowledge holds; so does the server's ending it, after
 * {@link RECONNECT_DELAY_MS}, giving up as `rejected` after three in a row with no push applied. A
 * notification's contacts replace those held, and its census the census, where it sends one; a
 * contact that has cleared is dropped, from a push or from the state opened with
 * ({@link contactLit}; P12.T8.c). On the link's loss everything held stays on show as `stale`. An
 * acknowledgement is closed-loop: it is `pending` until the server answers, and its acceptance
 * marks the contact acknowledged, since the server pushes the contact only where it changed.
 *
 * @param universe - The universe whose detections to hold, or `null` for none (`idle`).
 * @param observer - The display's observer and time, or `null` while it has none (`no_observer`).
 */
export function useAlerts(
  universe: UniverseIdHex | null,
  observer: AlertObserver | null,
): AlertsView {
  const { status, requests } = useServerLink();
  const connected = status === "connected";
  const [held, setHeld] = useState(() => new AlertsStore(requests, universe));
  let store = held;
  if (!held.serves(requests, universe)) {
    store = new AlertsStore(requests, universe);
    setHeld(store);
  }
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const x = observer?.positionLy[0] ?? null;
  const y = observer?.positionLy[1] ?? null;
  const z = observer?.positionLy[2] ?? null;
  const timeYr = observer?.timeYr ?? null;

  useEffect(() => {
    store.setObserver(
      x === null || y === null || z === null || timeYr === null
        ? null
        : { positionLy: [x, y, z], timeYr },
    );
  }, [store, x, y, z, timeYr]);

  useEffect(() => {
    if (store.universe === null) {
      return undefined;
    }
    if (!connected) {
      store.linkDown();
      return undefined;
    }
    store.open();
    return () => {
      store.close();
    };
  }, [store, connected]);

  return { ...snapshot, acknowledge: store.acknowledge, retry: store.retry };
}
