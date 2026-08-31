import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  findPhoneCountry,
  PHONE_COUNTRIES,
  type PhoneCountry,
} from '@/lib/phone-countries'

/**
 * The one phone input for the whole platform — the web twin of the
 * mobile app's phone screen: pick the country (flag + dial code, same
 * list, Rwanda first), then type the national digits. The submitted
 * value is always `dialCode + digits` (E.164), assembled by the caller
 * with `joinE164` — a phone can never reach the server without its
 * country code, and no real-looking number ever sits in a placeholder.
 */
export function CountryPhoneInput({
  id,
  country,
  onCountryChange,
  national,
  onNationalChange,
}: {
  id: string
  country: PhoneCountry
  onCountryChange: (country: PhoneCountry) => void
  /** National digits only — the dial code lives in the select. */
  national: string
  onNationalChange: (national: string) => void
}) {
  return (
    <div className="flex gap-2">
      <Select
        value={country.dialCode}
        onValueChange={(value) => {
          const picked = value != null ? findPhoneCountry(value) : undefined
          if (picked) onCountryChange(picked)
        }}
      >
        <SelectTrigger id={`${id}-country`} className="w-[7.75rem] shrink-0">
          {/* Base UI resolves item labels only while the popup is mounted —
              feed the closed trigger its own label. */}
          <SelectValue placeholder="Code">
            {country.flag} {country.dialCode}
          </SelectValue>
        </SelectTrigger>
        <SelectContent>
          {PHONE_COUNTRIES.map((entry) => (
            <SelectItem key={entry.dialCode} value={entry.dialCode}>
              {entry.flag} {entry.name} ({entry.dialCode})
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Input
        id={id}
        type="tel"
        inputMode="numeric"
        autoComplete="off"
        className="min-w-0 flex-1"
        value={national}
        onChange={(event) => onNationalChange(event.target.value.replace(/\D/g, ''))}
        placeholder="7XX XXX XXX"
      />
    </div>
  )
}
