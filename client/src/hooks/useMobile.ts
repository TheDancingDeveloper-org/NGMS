import { useSyncExternalStore } from 'react'

const MOBILE_BREAKPOINT = 768

function subscribe(callback: () => void): () => void {
  const mql = window.matchMedia(`(max-width: ${MOBILE_BREAKPOINT - 1}px)`)
  mql.addEventListener('change', callback)
  return () => mql.removeEventListener('change', callback)
}

function getSnapshot(): boolean {
  return window.innerWidth < MOBILE_BREAKPOINT
}

export function useMobile(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, () => false)
}

/** True when running inside Tauri on a mobile platform */
export function isTauriMobile(): boolean {
  return '__TAURI__' in window && /Android|iPhone|iPad/i.test(navigator.userAgent)
}
