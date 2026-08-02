import { useSyncExternalStore } from "react";

type Listener = () => void;

export interface SimpleStore<T> {
  getState: () => T;
  setState: (updater: T | ((state: T) => T)) => void;
  subscribe: (listener: Listener) => () => void;
}

export function createStore<T>(initialState: T): SimpleStore<T> {
  let state = initialState;
  const listeners = new Set<Listener>();

  return {
    getState: () => state,
    setState: (updater) => {
      state =
        typeof updater === "function"
          ? (updater as (currentState: T) => T)(state)
          : updater;
      listeners.forEach((listener) => listener());
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

export function useStore<T, Selected>(
  store: SimpleStore<T>,
  selector: (state: T) => Selected,
): Selected {
  return useSyncExternalStore(
    store.subscribe,
    () => selector(store.getState()),
    () => selector(store.getState()),
  );
}
