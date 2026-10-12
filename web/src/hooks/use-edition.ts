import { useQuery } from '@tanstack/react-query'
import axiosInstance from '@/api/axiosInstance'

export interface EditionInfo {
  edition: 'community' | 'pro' | 'enterprise'
  version: string
  sso_enabled: boolean
  oidc_only?: boolean
  ldap_enabled: boolean
  siem_enabled: boolean
  approval_enabled: boolean
}

async function fetchEdition(): Promise<EditionInfo> {
  const { data } = await axiosInstance.get<EditionInfo>('api/v1/features')
  return data
}

export function useEdition() {
  const { data, isPending, isError, refetch } = useQuery({
    queryKey: ['edition'],
    queryFn: fetchEdition,
    staleTime: Infinity,
    retry: 1,
  })

  return {
    isPro: data?.edition === 'pro' || data?.edition === 'enterprise',
    isEnterprise: data?.edition === 'enterprise',
    edition: data?.edition ?? 'community',
    version: data?.version ?? '',
    ssoEnabled: data?.sso_enabled ?? false,
    oidcOnly: data?.oidc_only ?? false,
    isPending,
    isError,
    refetch,
    ldapEnabled: data?.ldap_enabled ?? false,
    siemEnabled: data?.siem_enabled ?? false,
    approvalEnabled: data?.approval_enabled ?? false,
  } as const
}
