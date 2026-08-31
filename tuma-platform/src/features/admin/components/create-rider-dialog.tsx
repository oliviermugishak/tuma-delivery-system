import { useEffect, useState } from 'react'

import { CountryPhoneInput } from '@/components/country-phone-input'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { useCreateRider } from '@/features/admin/hooks/use-create-rider'
import {
  DEFAULT_PHONE_COUNTRY,
  joinE164,
  nationalDigitsProblem,
  type PhoneCountry,
} from '@/lib/phone-countries'

/**
 * "Add rider" dialog — onboards a Tuma driver: the OTP account (phone
 * only, no password — they sign in in the app with the customer OTP flow)
 * plus the rider profile, whose unique number is generated server-side.
 * The phone is entered country-picker first (the web twin of the mobile
 * phone screen) and always submitted in E.164. The mutation (toast + list
 * invalidation) lives in use-create-rider; this component owns the field
 * state and closes itself on success. On failure the dialog stays open
 * with the values intact so the admin can fix the input and resubmit.
 */
export function CreateRiderDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const createRider = useCreateRider()
  const [name, setName] = useState('')
  const [country, setCountry] = useState<PhoneCountry>(DEFAULT_PHONE_COUNTRY)
  const [national, setNational] = useState('')
  const [phoneError, setPhoneError] = useState<string | null>(null)

  // Fresh form every time the dialog opens.
  useEffect(() => {
    if (open) {
      setName('')
      setCountry(DEFAULT_PHONE_COUNTRY)
      setNational('')
      setPhoneError(null)
    }
  }, [open])

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    const problem = nationalDigitsProblem(country, national)
    if (problem) {
      setPhoneError(problem)
      return
    }
    createRider.mutate(
      { body: { name: name.trim(), phone: joinE164(country.dialCode, national) } },
      { onSuccess: () => onOpenChange(false) },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add rider</DialogTitle>
          <DialogDescription>
            Creates the rider&apos;s account and their unique rider number.
            They sign in in the app with their phone and the OTP code.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="rider-name">Name</FieldLabel>
              <Input
                id="rider-name"
                autoComplete="off"
                required
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Jean Bosco"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="rider-phone">Phone</FieldLabel>
              <CountryPhoneInput
                id="rider-phone"
                country={country}
                onCountryChange={(picked) => {
                  setCountry(picked)
                  setPhoneError(null)
                }}
                national={national}
                onNationalChange={(digits) => {
                  setNational(digits)
                  setPhoneError(null)
                }}
              />
              {phoneError ? (
                <FieldDescription className="text-destructive">
                  {phoneError}
                </FieldDescription>
              ) : (
                <FieldDescription>
                  Pick the country, then the number — the rider verifies
                  with it in the app.
                </FieldDescription>
              )}
            </Field>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={createRider.isPending}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={createRider.isPending}>
                {createRider.isPending ? 'Creating…' : 'Create rider'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
