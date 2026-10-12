//
// Copyright (c) 2025-2026 rustmailer.com (https://rustmailer.com)
//
// This file is part of the Bichon Email Archiving Project
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
import { useEffect, useRef } from 'react'
import { useLocation } from '@tanstack/react-router'
import { useTranslation } from 'react-i18next'
import { resolveApiUrl } from '@/api/branding/api'
import { toSearchParams } from '@/lib/utils'
import { useEdition } from '@/hooks/use-edition'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/button'
import { AuthLayout } from './auth-layout'
import { UserAuthForm } from './user-auth-form'

export default function SignIn() {
  const { t } = useTranslation()
  const { oidcOnly, isPending, isError, refetch } = useEdition()
  const { search } = useLocation()
  const started = useRef(false)
  const oidcLoginUrl = resolveApiUrl('/api/auth/oidc/login')
  const autoStartKey = `bichon.oidc.auto-start:${oidcLoginUrl}`
  const lastStart = oidcOnly ? Number(sessionStorage.getItem(autoStartKey)) : 0
  const manual =
    toSearchParams(search).get('oidc') === 'manual' ||
    (lastStart > 0 && Date.now() - lastStart < 60_000)

  useEffect(() => {
    if (oidcOnly && !isPending && !isError && !manual && !started.current) {
      started.current = true
      sessionStorage.setItem(autoStartKey, String(Date.now()))
      location.replace(oidcLoginUrl)
    }
  }, [oidcOnly, isPending, isError, manual, oidcLoginUrl, autoStartKey])

  return (
    <AuthLayout>
      <Card>
        <CardHeader>
          <CardTitle className='text-xl tracking-tight'>
            {t('auth.signInTitle', 'Sign in to your account')}
          </CardTitle>
          {!isPending && !isError && (
            <CardDescription>
              {oidcOnly
                ? t(
                    'auth.ssoSignInDescription',
                    'Continue with your identity provider.'
                  )
                : t(
                    'auth.signInDescription',
                    'Enter your credentials to continue.'
                  )}
            </CardDescription>
          )}
        </CardHeader>
        <CardContent>
          {isPending ? (
            <p role='status'>
              {t('auth.loadingSignIn', 'Loading sign-in settings...')}
            </p>
          ) : isError ? (
            <div className='grid gap-4'>
              <p role='alert'>
                {t(
                  'auth.signInSettingsFailed',
                  'Unable to load sign-in settings. Please try again.'
                )}
              </p>
              <Button onClick={() => void refetch()}>
                {t('auth.retrySignInSettings', 'Try again')}
              </Button>
            </div>
          ) : oidcOnly ? (
            <div className='grid gap-4'>
              {!manual && (
                <p role='status'>
                  {t(
                    'auth.redirectingToSso',
                    'Redirecting to your identity provider...'
                  )}
                </p>
              )}
              <Button asChild className='w-full'>
                <a href={oidcLoginUrl}>
                  {t('auth.signInWithSso', 'Sign in with SSO')}
                </a>
              </Button>
            </div>
          ) : (
            <UserAuthForm />
          )}
        </CardContent>
      </Card>
    </AuthLayout>
  )
}
