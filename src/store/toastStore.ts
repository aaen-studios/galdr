import { create } from "zustand";

export type ToastKind = "success" | "error" | "info" | "warn";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: string;
  kind: ToastKind;
  title: string;
  message?: string;
  action?: ToastAction;
}

/** A log entry — same shape as Toast but with a timestamp, for the notification center. */
export interface NotificationEntry extends Toast {
  timestamp: number;
}

/** Fields a caller provides — `id` and auto-dismiss are handled by the store. */
export type ToastInput = Omit<Toast, "id">;

interface ToastState {
  toasts: Toast[];
  /** Persistent log of all notifications (notification center). Capped. */
  log: NotificationEntry[];
  /** Unread count for the titlebar badge. */
  unread: number;
  /** Push a toast. Auto-dismisses after the kind's timeout unless it's an
   *  error (errors persist until dismissed — the user should acknowledge them).
   *  Returns the generated id so callers can update/remove it later.
   *  Also appends to the persistent log. */
  push: (input: ToastInput) => string;
  /** Remove a toast by id. Safe to call with an unknown id. */
  dismiss: (id: string) => void;
  /** Remove every toast. */
  clear: () => void;
  /** Clear the notification log. */
  clearLog: () => void;
  /** Mark all notifications as read (resets unread badge). */
  markRead: () => void;
}

/** Maximum toasts kept on screen at once. Older ones are dropped first. */
const MAX_TOASTS = 4;

/** Maximum entries in the persistent notification log. */
const MAX_LOG = 100;

/** Auto-dismiss delay (ms) for non-error toasts. */
const AUTO_DISMISS_MS = 5000;

/** Track pending timers so a manual dismiss can cancel the scheduled removal. */
const timers = new Map<string, ReturnType<typeof setTimeout>>();

function clearTimer(id: string) {
  const t = timers.get(id);
  if (t !== undefined) {
    clearTimeout(t);
    timers.delete(id);
  }
}

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  log: [],
  unread: 0,

  push: (input) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const toast: Toast = { id, ...input };
    const entry: NotificationEntry = { ...toast, timestamp: Date.now() };

    set((state) => {
      // Drop the oldest when over capacity. Slice first so the new toast
      // always lands at the end (bottom of the stack).
      const next = [...state.toasts, toast];
      if (next.length > MAX_TOASTS) {
        const dropped = next.slice(0, next.length - MAX_TOASTS);
        dropped.forEach((d) => clearTimer(d.id));
      }
      const toasts = next.length > MAX_TOASTS ? next.slice(next.length - MAX_TOASTS) : next;
      // Append to the persistent log (newest first), capped.
      const log = [entry, ...state.log].slice(0, MAX_LOG);
      return { toasts, log, unread: state.unread + 1 };
    });

    // Errors persist until dismissed; everything else auto-dismisses.
    if (toast.kind !== "error") {
      const timer = setTimeout(() => {
        timers.delete(id);
        get().dismiss(id);
      }, AUTO_DISMISS_MS);
      timers.set(id, timer);
    }

    return id;
  },

  dismiss: (id) => {
    clearTimer(id);
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
  },

  clear: () => {
    get().toasts.forEach((t) => clearTimer(t.id));
    set({ toasts: [] });
  },

  clearLog: () => set({ log: [], unread: 0 }),

  markRead: () => set({ unread: 0 }),
}));
