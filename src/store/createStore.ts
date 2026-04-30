import { useRef, useSyncExternalStore } from 'react'

type Listener = () => void
type StoreUpdater<TState> = TState | ((state: TState) => TState)
type EqualityFn<TValue> = (left: TValue, right: TValue) => boolean

export interface SimpleStore<TState> {
  getState: () => TState
  setState: (updater: StoreUpdater<TState>) => void
  subscribe: (listener: Listener) => () => void
}

const defaultEquality: EqualityFn<unknown> = Object.is

export function createStore<TState>(initialState: TState): SimpleStore<TState> {
  let state = initialState
  const listeners = new Set<Listener>()

  return {
    getState: () => state,
    setState: (updater) => {
      const nextState =
        typeof updater === 'function'
          ? (updater as (state: TState) => TState)(state)
          : updater

      if (Object.is(state, nextState)) {
        return
      }

      state = nextState
      listeners.forEach((listener) => listener())
    },
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}

export function useStoreSelector<TState, TValue>(
  store: SimpleStore<TState>,
  selector: (state: TState) => TValue,
  isEqual: EqualityFn<TValue> = defaultEquality as EqualityFn<TValue>
): TValue {
  const snapshotRef = useRef<TValue | undefined>(undefined)

  return useSyncExternalStore(
    store.subscribe,
    () => {
      const nextValue = selector(store.getState())
      const previousValue = snapshotRef.current

      if (previousValue !== undefined && isEqual(previousValue, nextValue)) {
        return previousValue
      }

      snapshotRef.current = nextValue
      return nextValue
    },
    () => selector(store.getState())
  )
}

export function createRequestGate() {
  let activeRequestId = 0
  let activeController: AbortController | null = null

  return {
    next() {
      activeController?.abort()
      activeRequestId += 1
      const requestId = activeRequestId
      activeController = new AbortController()

      return {
        requestId,
        signal: activeController.signal,
        isCurrent: () => requestId === activeRequestId,
      }
    },
    isCurrent(requestId: number) {
      return requestId === activeRequestId
    },
    cancel() {
      activeRequestId += 1
      activeController?.abort()
      activeController = null
    },
  }
}
