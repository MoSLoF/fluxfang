import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import Setup from './Setup';
import { mockFetchRoutes } from '../test-utils/fetchMocks';

afterEach(() => vi.unstubAllGlobals());

function setupFetch(opts: { requiresToken?: boolean } = {}) {
  return mockFetchRoutes({
    '/api/setup/status': { needs_setup: true, requires_token: opts.requiresToken ?? false },
    '/api/setup': undefined,
  });
}

test('defaults to Standalone and posts role + default node id', async () => {
  const fetchMock = setupFetch();
  vi.stubGlobal('fetch', fetchMock);
  render(<Setup onSetupComplete={vi.fn()} />);

  fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'pw123456' } });
  fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'pw123456' } });
  fireEvent.click(screen.getByRole('button', { name: /finish setup/i }));

  await waitFor(() => {
    const setupCalls = fetchMock.mock.calls.filter(
      ([url]) => new URL(String(url), 'http://localhost').pathname === '/api/setup',
    );
    expect(setupCalls.length).toBe(1);
  });
  const setupCall = fetchMock.mock.calls.find(
    ([url]) => new URL(String(url), 'http://localhost').pathname === '/api/setup',
  );
  const body = JSON.parse(setupCall![1]!.body as string);
  expect(body.role).toBe('standalone');
  expect(body.node_sensor_id).toBe('local');
  expect(body.sensor).toBeUndefined();
});

test('choosing Sensor reveals connection fields and posts them', async () => {
  const fetchMock = setupFetch();
  vi.stubGlobal('fetch', fetchMock);
  render(<Setup onSetupComplete={vi.fn()} />);

  fireEvent.click(screen.getByRole('radio', { name: /sensor/i }));

  fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'pw123456' } });
  fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'pw123456' } });
  fireEvent.change(screen.getByLabelText(/sensor id/i), { target: { value: 'frontgate' } });
  fireEvent.change(screen.getByLabelText(/host/i), { target: { value: 'base.example' } });
  fireEvent.change(screen.getByLabelText(/port/i), { target: { value: '9000' } });
  fireEvent.change(screen.getByLabelText(/encryption key/i), { target: { value: 'a2V5' } });

  fireEvent.click(screen.getByRole('button', { name: /finish setup/i }));

  await waitFor(() => {
    const setupCalls = fetchMock.mock.calls.filter(
      ([url]) => new URL(String(url), 'http://localhost').pathname === '/api/setup',
    );
    expect(setupCalls.length).toBe(1);
  });
  const setupCall = fetchMock.mock.calls.find(
    ([url]) => new URL(String(url), 'http://localhost').pathname === '/api/setup',
  );
  const body = JSON.parse(setupCall![1]!.body as string);
  expect(body.role).toBe('sensor');
  expect(body.node_sensor_id).toBe('frontgate');
  expect(body.sensor).toMatchObject({ host: 'base.example', port: 9000, key: 'a2V5' });
});

test('Generate fills the encryption key field', async () => {
  vi.stubGlobal('fetch', setupFetch());
  render(<Setup onSetupComplete={vi.fn()} />);
  fireEvent.click(screen.getByRole('radio', { name: /sensor/i }));

  const keyField = screen.getByLabelText(/encryption key/i) as HTMLInputElement;
  expect(keyField.value).toBe('');
  fireEvent.click(screen.getByRole('button', { name: /generate/i }));
  expect(keyField.value.length).toBeGreaterThan(0);
});

test('rejects a sensor id containing a space', async () => {
  const fetchMock = setupFetch();
  vi.stubGlobal('fetch', fetchMock);
  render(<Setup onSetupComplete={vi.fn()} />);
  fireEvent.click(screen.getByRole('radio', { name: /sensor/i }));
  fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'pw123456' } });
  fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'pw123456' } });
  fireEvent.change(screen.getByLabelText(/sensor id/i), { target: { value: 'front gate' } });
  fireEvent.click(screen.getByRole('button', { name: /finish setup/i }));

  expect(await screen.findByRole('alert')).toHaveTextContent(/id/i);
  const setupCalls = fetchMock.mock.calls.filter(
    ([url]) => new URL(String(url), 'http://localhost').pathname === '/api/setup',
  );
  expect(setupCalls.length).toBe(0);
});

test('shows bootstrap token field when requires_token is true', async () => {
  vi.stubGlobal('fetch', setupFetch({ requiresToken: true }));
  render(<Setup onSetupComplete={vi.fn()} />);

  expect(await screen.findByLabelText(/bootstrap token/i)).toBeInTheDocument();
});

test('includes bootstrap_token in setup request when required', async () => {
  const fetchMock = setupFetch({ requiresToken: true });
  vi.stubGlobal('fetch', fetchMock);
  render(<Setup onSetupComplete={vi.fn()} />);

  await screen.findByLabelText(/bootstrap token/i);
  fireEvent.change(screen.getByLabelText(/bootstrap token/i), { target: { value: 'test-token-123' } });
  fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'pw123456' } });
  fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'pw123456' } });
  fireEvent.click(screen.getByRole('button', { name: /finish setup/i }));

  await waitFor(() => {
    const setupCalls = fetchMock.mock.calls.filter(
      ([url]) => new URL(String(url), 'http://localhost').pathname === '/api/setup',
    );
    expect(setupCalls.length).toBe(1);
  });
  const setupCall = fetchMock.mock.calls.find(
    ([url]) => new URL(String(url), 'http://localhost').pathname === '/api/setup',
  );
  const body = JSON.parse(setupCall![1]!.body as string);
  expect(body.bootstrap_token).toBe('test-token-123');
});

test('shows error when token required but not provided', async () => {
  vi.stubGlobal('fetch', setupFetch({ requiresToken: true }));
  render(<Setup onSetupComplete={vi.fn()} />);

  await screen.findByLabelText(/bootstrap token/i);
  fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'pw123456' } });
  fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'pw123456' } });
  fireEvent.click(screen.getByRole('button', { name: /finish setup/i }));

  expect(await screen.findByRole('alert')).toHaveTextContent(/bootstrap token/i);
});
