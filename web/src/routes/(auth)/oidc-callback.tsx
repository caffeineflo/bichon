// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 Florian Harr
// Added October 10, 2026.
import { useCallback, useEffect, useRef, useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import i18n from '@/i18n'
import { resolveApiUrl } from '@/api/branding/api'
import { type LoginResult, mfaVerify } from '@/api/users/api'
import { setToken } from '@/stores/authStore'
import { useTheme } from '@/context/theme-context'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/button'
import { AuthLayout } from '@/features/auth/auth-layout'

export const Route = createFileRoute('/(auth)/oidc-callback')({
  component: OidcCallback,
})

function OidcCallback() {
  const navigate = useNavigate()
  const { setTheme } = useTheme()
  const started = useRef(false)
  const [error, setError] = useState('')
  const [challenge, setChallenge] = useState<string | null>(null)
  const [code, setCode] = useState('')
  const [busy, setBusy] = useState(false)

  const finish = useCallback(
    async (result: LoginResult) => {
      if (result.mfa_required && result.mfa_challenge) {
        setChallenge(result.mfa_challenge)
        return
      }
      if (!result.success || !result.access_token) {
        throw new Error(result.error_message || 'Sign-in failed.')
      }
      setToken(result)
      if (result.theme) setTheme(result.theme)
      if (result.language) await i18n.changeLanguage(result.language)
      await navigate({ to: '/', replace: true })
    },
    [navigate, setTheme]
  )

  useEffect(() => {
    if (started.current) return
    started.current = true
    async function exchange() {
      try {
        const response = await fetch(resolveApiUrl('/api/auth/oidc/session'), {
          method: 'POST',
          credentials: 'same-origin',
        })
        if (!response.ok)
          throw new Error('Sign-in has expired or failed. Please try again.')
        await finish((await response.json()) as LoginResult)
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : 'Sign-in failed.')
      }
    }
    void exchange()
  }, [finish])

  return (
    <AuthLayout>
      <div className='grid gap-4'>
        {error ? (
          <p role='alert'>{error}</p>
        ) : (
          <p>
            {challenge
              ? 'Enter your Bichon authentication code.'
              : 'Completing sign-in...'}
          </p>
        )}
        {challenge && (
          <form
            className='grid gap-3'
            onSubmit={async (event) => {
              event.preventDefault()
              setBusy(true)
              setError('')
              try {
                await finish(await mfaVerify(challenge, code))
              } catch (cause) {
                setError(
                  cause instanceof Error
                    ? cause.message
                    : 'Verification failed.'
                )
              } finally {
                setBusy(false)
              }
            }}
          >
            <Input
              aria-label='Authentication code'
              inputMode='numeric'
              autoComplete='one-time-code'
              value={code}
              onChange={(event) => setCode(event.target.value)}
            />
            <Button disabled={busy}>Verify</Button>
          </form>
        )}
        {error && (
          <Button
            variant='outline'
            onClick={() =>
              void navigate({ to: '/sign-in', search: { oidc: 'manual' } })
            }
          >
            Return to sign-in
          </Button>
        )}
      </div>
    </AuthLayout>
  )
}
