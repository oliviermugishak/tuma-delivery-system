import { useEffect, useState } from 'react'

import type { RiderAdminResponse } from '@/api/generated'
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
import { useUpdateRider } from '@/features/admin/hooks/use-update-rider'
import {
  DEFAULT_PHONE_COUNTRY,
  joinE164,
  nationalDigitsProblem,
  splitE164,
  type PhoneCountry,
} from '@/lib/phone-countries'

/**
 * "Edit rider" dialog: name + phone. The phone is the rider's OTP anchor —
 * editing it is how an admin fixes a typo'd number from creation (the
 * server rejects a phone another account already holds with a 409). The
 * phone is entered country-picker first and always submitted in E.164;
 * only changed fields are sent; the name is required (riders have no
 * clear-to-null — the server 400s an empty one). Closes on success; stays
 * open on failure with values intact.
 */
export function EditRiderDialog({
  rider,
  open,
  onOpenChange,
}: {
  rider: RiderAdminResponse | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const updateRider = useUpdateRider()
  const [name, setName] = useState('')
  const [country, setCountry] = useState<PhoneCountry>(DEFAULT_PHONE_COUNTRY)
  const [national, setNational] = useState('')
  const [phoneError, setPhoneError] = useState<string | null>(null)

  // Seed the form from the rider each time the dialog opens — the stored
  // E.164 splits back into country + digits.
  useEffect(() => {
    if (open && rider) {
      setName(rider.name)
      const split = splitE164(rider.phone)
      setCountry(split.country)
      setNational(split.national)
      setPhoneError(null)
    }
  }, [open, rider])

  if (!rider) return null

  const nameChanged = name.trim() !== rider.name
  const phoneChanged = joinE164(country.dialCode, national) !== rider.phone
  const dirty = nameChanged || phoneChanged

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!dirty || !name.trim()) return
    const problem = nationalDigitsProblem(country, national)
    if (problem) {
      setPhoneError(problem)
      return
    }
    const body: Record<string, string> = {}
    if (nameChanged) body.name = name.trim()
    if (phoneChanged) body.phone = joinE164(country.dialCode, national)
    updateRider.mutate(
      { path: { id: rider.id }, body },
      { onSuccess: () => onOpenChange(false) },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit rider</DialogTitle>
          <DialogDescription>
            Update the rider&apos;s identity. The rider number never changes.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="edit-rider-name">Name</FieldLabel>
              <Input
                id="edit-rider-name"
                autoComplete="off"
                required
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Jean Bosco"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-rider-phone">Phone</FieldLabel>
              <CountryPhoneInput
                id="edit-rider-phone"
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
                  The number they verify with in the app.
                </FieldDescription>
              )}
            </Field>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={updateRider.isPending}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={!dirty || !name.trim() || updateRider.isPending}
              >
                {updateRider.isPending ? 'Saving…' : 'Save changes'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
