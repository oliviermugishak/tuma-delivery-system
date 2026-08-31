// Phone country data + the E.164 assemble/parse pair. The single source of
// truth for phone entry across the platform — every phone input renders
// this list, and every submitted value is `dialCode + national digits`.
// Mirrors the mobile app's list (lib/features/auth/phone_screen.dart) so
// both clients speak the same phone dialect.

export interface PhoneCountry {
  name: string
  flag: string
  dialCode: string
  /** Exact national-number length when known; otherwise 8–12 digits pass. */
  nationalDigits?: number
}

export const PHONE_COUNTRIES: PhoneCountry[] = [
  { name: 'Rwanda', flag: '🇷🇼', dialCode: '+250', nationalDigits: 9 },
  { name: 'Kenya', flag: '🇰🇪', dialCode: '+254' },
  { name: 'Uganda', flag: '🇺🇬', dialCode: '+256' },
  { name: 'Tanzania', flag: '🇹🇿', dialCode: '+255' },
  { name: 'Burundi', flag: '🇧🇮', dialCode: '+257' },
  { name: 'DR Congo', flag: '🇨🇩', dialCode: '+243' },
]

/** Rwanda first — the platform's home market. */
export const DEFAULT_PHONE_COUNTRY = PHONE_COUNTRIES[0]!

export function findPhoneCountry(dialCode: string): PhoneCountry | undefined {
  return PHONE_COUNTRIES.find((country) => country.dialCode === dialCode)
}

/**
 * Split a stored E.164 phone into its country + national digits for
 * seeding an input. Unknown or malformed values keep the default country
 * and leave the digits empty — the admin re-picks; nothing is guessed.
 */
export function splitE164(
  phone: string | null | undefined,
): { country: PhoneCountry; national: string } {
  if (!phone || !phone.startsWith('+')) {
    return { country: DEFAULT_PHONE_COUNTRY, national: '' }
  }
  // Longest dial code first, so +250 wins over a hypothetical +25.
  const match = [...PHONE_COUNTRIES]
    .sort((a, b) => b.dialCode.length - a.dialCode.length)
    .find((country) => phone.startsWith(country.dialCode))
  if (!match) {
    return { country: DEFAULT_PHONE_COUNTRY, national: '' }
  }
  return { country: match, national: phone.slice(match.dialCode.length) }
}

/** `+250` + `783002002` → `+250783002002` — the only assembly rule. */
export function joinE164(dialCode: string, national: string): string {
  return `${dialCode}${national}`
}

/**
 * The gate every phone passes before submission: national digits must be
 * all digits and match the picked country's known length (or 8–12 when
 * unknown). The dial code comes from the select, so it is always valid —
 * the input never carries it.
 */
export function nationalDigitsProblem(
  country: PhoneCountry,
  national: string,
): string | null {
  const digits = national.replace(/\D/g, '')
  if (digits.length === 0) return 'Enter the number.'
  const expected = country.nationalDigits
  if (expected != null) {
    if (digits.length !== expected) {
      return `Enter a ${expected}-digit number for ${country.name}.`
    }
  } else if (digits.length < 8 || digits.length > 12) {
    return 'Enter a valid phone number.'
  }
  return null
}
