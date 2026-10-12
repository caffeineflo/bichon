import { StrictMode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Route as OidcCallbackRoute } from '@/routes/(auth)/oidc-callback'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SignOutDialog } from '@/components/sign-out-dialog'
import SignIn from '..'

const state = vi.hoisted(() => ({
  oidcOnly: true,
  isPending: false,
  isError: false,
  search: {} as Record<string, string>,
  replace: vi.fn(),
  refetch: vi.fn(),
  navigate: vi.fn(),
}))
const session = new Map<string, string>()

vi.mock('@/hooks/use-edition', () => ({
  useEdition: () => ({
    ...state,
    ssoEnabled: state.oidcOnly,
    ldapEnabled: false,
  }),
}))
vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tanstack/react-router')>()),
  useLocation: () => ({ search: state.search, href: '/archive/messages' }),
  useNavigate: () => state.navigate,
}))
vi.mock('../auth-layout', () => ({
  AuthLayout: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}))
vi.mock('@/context/theme-context', () => ({
  useTheme: () => ({ setTheme: vi.fn() }),
}))
vi.mock('@/stores/authStore', () => ({
  getToken: () => null,
  resetToken: vi.fn(),
  setToken: vi.fn(),
}))
vi.mock('@/api/branding/api', () => ({
  resolveApiUrl: (path: string) => `/archive${path}`,
}))
vi.mock('react-i18next', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-i18next')>()
  const labels: Record<string, string> = {
    'auth.password': 'Password',
    'auth.username': 'Username',
  }
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string, fallback?: string) => fallback || labels[key] || key,
    }),
  }
})

function page(content: React.ReactNode = <SignIn />) {
  return (
    <QueryClientProvider client={new QueryClient()}>
      <StrictMode>{content}</StrictMode>
    </QueryClientProvider>
  )
}

beforeEach(() => {
  state.oidcOnly = true
  state.isPending = false
  state.isError = false
  state.search = {}
  session.clear()
  vi.clearAllMocks()
  vi.stubGlobal('location', { replace: state.replace })
  vi.stubGlobal('sessionStorage', {
    getItem: (key: string) => session.get(key) ?? null,
    setItem: (key: string, value: string) => session.set(key, value),
  })
})
afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('OIDC-only sign-in', () => {
  it('starts SSO once and never offers a password form', () => {
    render(page())
    expect(state.replace).toHaveBeenCalledExactlyOnceWith(
      '/archive/api/auth/oidc/login'
    )
    expect(screen.queryByLabelText('Password')).not.toBeInTheDocument()
  })

  it('stops rapid automatic SSO across full-page remounts', () => {
    const first = render(page())
    first.unmount()
    render(page())
    expect(state.replace).toHaveBeenCalledTimes(1)
    expect(
      screen.getByRole('link', { name: 'Sign in with SSO' })
    ).toBeInTheDocument()
  })

  it('automatically signs in again after the cooldown has expired', () => {
    const now = vi.spyOn(Date, 'now').mockReturnValue(1_800_000_000_000)
    const first = render(page())
    first.unmount()
    now.mockReturnValue(1_800_000_060_001)
    render(page())
    expect(state.replace).toHaveBeenCalledTimes(2)
  })

  it('waits for the server auth mode before showing any login option', () => {
    state.isPending = true
    state.oidcOnly = false
    const view = render(page())
    expect(screen.queryByLabelText('Password')).not.toBeInTheDocument()
    expect(state.replace).not.toHaveBeenCalled()
    state.isPending = false
    state.oidcOnly = true
    view.rerender(page())
    expect(state.replace).toHaveBeenCalledExactlyOnceWith(
      '/archive/api/auth/oidc/login'
    )
  })

  it('does not redirect again after sign-out or a failed callback', () => {
    state.search = { oidc: 'manual' }
    render(page())
    expect(state.replace).not.toHaveBeenCalled()
    expect(
      screen.getByRole('link', { name: 'Sign in with SSO' })
    ).toHaveAttribute('href', '/archive/api/auth/oidc/login')
    expect(screen.queryByLabelText('Password')).not.toBeInTheDocument()
  })

  it('offers a settings retry instead of falling back to password on failure', () => {
    state.isError = true
    state.oidcOnly = false
    render(page())
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Unable to load sign-in settings'
    )
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    expect(state.refetch).toHaveBeenCalledOnce()
    expect(screen.queryByLabelText('Password')).not.toBeInTheDocument()
  })

  it('keeps local sign-in when the server has no OIDC configured', () => {
    state.oidcOnly = false
    render(page())
    expect(screen.getByLabelText('Password')).toBeInTheDocument()
    expect(state.replace).not.toHaveBeenCalled()
  })

  it('returns from a failed callback without starting SSO automatically', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('', { status: 401 }))
    )
    const Callback = OidcCallbackRoute.options.component!
    render(page(<Callback />))
    fireEvent.click(
      await screen.findByRole('button', { name: 'Return to sign-in' })
    )
    expect(state.navigate).toHaveBeenCalledWith({
      to: '/sign-in',
      search: { oidc: 'manual' },
    })
  })

  it('signs out to the manual SSO page rather than immediately signing back in', () => {
    render(page(<SignOutDialog open onOpenChange={vi.fn()} />))
    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }))
    expect(state.navigate).toHaveBeenCalledWith({
      to: '/sign-in',
      search: { redirect: '/archive/messages', oidc: 'manual' },
      replace: true,
    })
  })
})
