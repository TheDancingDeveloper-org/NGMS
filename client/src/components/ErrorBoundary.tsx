// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { Component, type ReactNode } from 'react'
import type { ErrorInfo } from 'react'

interface Props {
  children: ReactNode
}

interface State {
  hasError: boolean
  error: Error | null
}

export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, info.componentStack)
  }

  render() {
    if (this.state.hasError) {
      return (
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          minHeight: '100vh',
          background: '#0f172a',
          color: '#e2e8f0',
          padding: 24,
          textAlign: 'center',
        }}>
          <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 12 }}>
            Something went wrong
          </h1>
          <p style={{ color: '#94a3b8', fontSize: 14, marginBottom: 24, maxWidth: 480 }}>
            An unexpected error occurred. Try reloading the page.
          </p>
          {import.meta.env.DEV && this.state.error && (
            <pre style={{
              background: '#1e293b',
              border: '1px solid #334155',
              borderRadius: 8,
              padding: 16,
              marginBottom: 24,
              maxWidth: 600,
              width: '100%',
              overflow: 'auto',
              fontSize: 12,
              color: '#f87171',
              textAlign: 'left',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
            }}>
              {this.state.error.message}
              {this.state.error.stack && `\n\n${this.state.error.stack}`}
            </pre>
          )}
          <button
            onClick={() => window.location.reload()}
            style={{
              background: '#1e40af',
              color: '#fff',
              border: 'none',
              borderRadius: 8,
              padding: '10px 24px',
              fontSize: 14,
              fontWeight: 600,
              cursor: 'pointer',
            }}
          >
            Reload
          </button>
        </div>
      )
    }

    return this.props.children
  }
}
