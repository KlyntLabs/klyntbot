/**
 * Tests for the useApi hook (lib/hooks/useApi.ts).
 *
 * Covers:
 * - AC-17.4: Hook exposes data, loading, error, refetch
 * - Loading state management
 * - Error state on API failure
 * - Successful data fetching and state update
 * - refetch() triggers a new API call
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { useApi } from '../useApi';
import { ApiError } from '../../api';

describe('useApi', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  // ── Initial state ─────────────────────────────────────────────────────────

  it('initially has data = undefined, loading = true, error = null', () => {
    // AC-17.4: Before the first fetch resolves, initial state is loading
    vi.stubGlobal('fetch', vi.fn().mockReturnValue(new Promise(() => {}))); // Never resolves
    const { result } = renderHook(() => useApi<{ id: string }[]>('/api/tasks'));
    expect(result.current.data).toBeUndefined();
    expect(result.current.loading).toBe(true);
    expect(result.current.error).toBeNull();
  });

  // ── Success ───────────────────────────────────────────────────────────────

  it('sets data and clears loading on successful fetch', async () => {
    // AC-17.4: After fetch resolves, data = response, loading = false
    const mockData = [{ id: '1', title: 'Test' }];
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => mockData,
    }));
    const { result } = renderHook(() => useApi<typeof mockData>('/api/tasks'));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.data).toEqual(mockData);
    expect(result.current.error).toBeNull();
  });

  it('sets loading = false after fetch regardless of outcome', async () => {
    // AC-17.4: Loading always becomes false after fetch completes (success or error)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
      json: async () => ({ message: 'Not found' }),
    }));
    const { result } = renderHook(() => useApi('/api/tasks/nonexistent'));
    await waitFor(() => expect(result.current.loading).toBe(false));
  });

  it('fetches from the provided URL', async () => {
    // AC-17.4: The hook calls apiFetch with the given URL
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => [],
    });
    vi.stubGlobal('fetch', fetchMock);
    renderHook(() => useApi('/api/tasks'));
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(fetchMock.mock.calls[0][0]).toContain('/api/tasks');
  });

  it('passes query params to the API', async () => {
    // AC-17.4: useApi('/api/tasks', { params: { status: 'todo' } }) passes params
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => [],
    });
    vi.stubGlobal('fetch', fetchMock);
    renderHook(() => useApi('/api/tasks', { params: { status: 'todo' } }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(fetchMock.mock.calls[0][0]).toContain('status=todo');
  });

  // ── Error state ───────────────────────────────────────────────────────────

  it('sets error when fetch fails with ApiError', async () => {
    // AC-17.4: On API failure, error = ApiError, data = undefined
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
      json: async () => ({ message: 'Not found' }),
    }));
    const { result } = renderHook(() => useApi('/api/tasks/nonexistent'));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toBeInstanceOf(ApiError);
    expect(result.current.data).toBeUndefined();
  });

  it('sets loading = false and error on fetch failure', async () => {
    // AC-17.4: After failure, loading = false, error has the error details
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: false,
      status: 500,
      json: async () => ({ message: 'Server error' }),
    }));
    const { result } = renderHook(() => useApi('/api/tasks'));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).not.toBeNull();
  });

  it('keeps stale data on refetch failure', async () => {
    // UX §3.4: If refetch fails after previous success, stale data is preserved
    const mockData = [{ id: '1' }];
    let callCount = 0;
    vi.stubGlobal('fetch', vi.fn().mockImplementation(() => {
      callCount++;
      if (callCount === 1) {
        return Promise.resolve({
          ok: true,
          status: 200,
          json: async () => mockData,
        });
      }
      return Promise.resolve({
        ok: false,
        status: 500,
        json: async () => ({ message: 'Server error' }),
      });
    }));
    const { result } = renderHook(() => useApi<typeof mockData>('/api/tasks'));
    await waitFor(() => expect(result.current.data).toEqual(mockData));
    act(() => result.current.refetch());
    await waitFor(() => expect(result.current.error).not.toBeNull());
    // Stale data should be preserved
    expect(result.current.data).toEqual(mockData);
  });

  // ── refetch ───────────────────────────────────────────────────────────────

  it('refetch() triggers a new API call', async () => {
    // AC-17.4: Calling refetch() re-fetches the data
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => [],
    });
    vi.stubGlobal('fetch', fetchMock);
    const { result } = renderHook(() => useApi('/api/tasks'));
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => result.current.refetch());
    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
  });

  it('refetch() sets loading = true while fetching', async () => {
    // AC-17.4: During refetch, loading is true
    let resolveSecond!: (value: unknown) => void;
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => [],
      })
      .mockReturnValueOnce(new Promise(resolve => { resolveSecond = resolve; }));
    vi.stubGlobal('fetch', fetchMock);
    const { result } = renderHook(() => useApi('/api/tasks'));
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => result.current.refetch());
    expect(result.current.loading).toBe(true);
    // Resolve to clean up
    act(() => resolveSecond({ ok: true, status: 200, json: async () => [] }));
  });

  // ── Loading timeout ───────────────────────────────────────────────────────

  it('shows error state after 5 seconds without data', async () => {
    // UX §3.2: Loading states appear for max 5 seconds — after that, show error
    vi.useFakeTimers();
    vi.stubGlobal('fetch', vi.fn().mockReturnValue(new Promise(() => {}))); // Never resolves
    const { result } = renderHook(() => useApi('/api/tasks'));
    expect(result.current.loading).toBe(true);
    // Use async act to flush React state updates triggered by the timer
    await act(async () => {
      vi.advanceTimersByTime(5001);
    });
    expect(result.current.error).not.toBeNull();
    vi.useRealTimers();
  });

  // ── Cleanup ───────────────────────────────────────────────────────────────

  it('aborts fetch on component unmount', async () => {
    // Edge: useApi uses AbortController to cancel in-flight fetches on unmount
    let capturedSignal: AbortSignal | undefined;
    vi.stubGlobal('fetch', vi.fn().mockImplementation((_url: unknown, opts: RequestInit) => {
      capturedSignal = opts.signal ?? undefined;
      return new Promise(() => {}); // Never resolves
    }));
    const { unmount } = renderHook(() => useApi('/api/tasks'));
    await waitFor(() => expect(capturedSignal).toBeDefined());
    expect(capturedSignal?.aborted).toBe(false);
    unmount();
    expect(capturedSignal?.aborted).toBe(true);
  });
});
