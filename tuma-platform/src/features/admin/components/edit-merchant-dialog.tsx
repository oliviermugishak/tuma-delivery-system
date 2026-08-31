import { useEffect, useState } from 'react'

import type { MerchantResponse } from '@/api/generated'
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
import { useUpdateMerchant } from '@/features/admin/hooks/use-update-merchant'
import {
  DEFAULT_PHONE_COUNTRY,
  joinE164,
  nationalDigitsProblem,
  splitE164,
  type PhoneCountry,
} from '@/lib/phone-countries'

/**
 * "Edit merchant" dialog: name + email + business phone. Password resets
 * are deliberately out — an admin who needs to reset one recreates the
 * account. The phone is entered country-picker first and always submitted
 * in E.164; leaving it empty clears it (an optional contact field). PATCH
 * semantics: only changed fields are sent; an emptied name clears it.
 * Closes on success; on failure (e.g. a taken email) it stays open with
 * the values intact so the admin can fix the input and resubmit.
 */
export function EditMerchantDialog({
  merchant,
  open,
  onOpenChange,
}: {
  merchant: MerchantResponse | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const updateMerchant = useUpdateMerchant()
  const [name, setName] = useState('')
  const [businessEmail, setBusinessEmail] = useState('')
  const [country, setCountry] = useState<PhoneCountry>(DEFAULT_PHONE_COUNTRY)
  const [national, setNational] = useState('')
  const [phoneError, setPhoneError] = useState<string | null>(null)

  // Seed the form from the business each time the dialog opens — the
  // stored E.164 splits back into country + digits.
  useEffect(() => {
    if (open && merchant) {
      setName(merchant.name)
      setBusinessEmail(merchant.business_email ?? '')
      const split = splitE164(merchant.business_phone)
      setCountry(split.country)
      setNational(split.national)
      setPhoneError(null)
    }
  }, [open, merchant])

  if (!merchant) return null

  const nameChanged = name.trim() !== merchant.name
  const businessEmailChanged =
    businessEmail.trim() !== (merchant.business_email ?? '')
  // An empty national field means "no phone" — the clear path.
  const phone = national ? joinE164(country.dialCode, national) : ''
  const businessPhoneChanged = phone !== (merchant.business_phone ?? '')
  const dirty = nameChanged || businessEmailChanged || businessPhoneChanged

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!dirty || !name.trim()) return
    if (businessPhoneChanged && national) {
      const problem = nationalDigitsProblem(country, national)
      if (problem) {
        setPhoneError(problem)
        return
      }
    }
    const body: {
      name?: string
      business_email?: string | null
      business_phone?: string | null
      status: string
    } = {
      // The generated PATCH carries status as a required field — echo the
      // current one so an identity edit never accidentally suspends.
      status: merchant.status,
    }
    if (nameChanged) body.name = name.trim()
    if (businessEmailChanged) {
      body.business_email = businessEmail.trim() || null
    }
    if (businessPhoneChanged) {
      body.business_phone = national ? joinE164(country.dialCode, national) : null
    }
    updateMerchant.mutate(
      { path: { id: merchant.id }, body },
      { onSuccess: () => onOpenChange(false) },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit merchant</DialogTitle>
          <DialogDescription>
            The business&apos;s identity and contact details. Owner sign-ins
            are managed per account, not here.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="edit-merchant-name">Name</FieldLabel>
              <Input
                id="edit-merchant-name"
                autoComplete="off"
                maxLength={100}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Acme Café"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-merchant-business-email">
                Business email
              </FieldLabel>
              <Input
                id="edit-merchant-business-email"
                type="email"
                autoComplete="off"
                value={businessEmail}
                onChange={(e) => setBusinessEmail(e.target.value)}
                placeholder="hello@acme.rw"
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-merchant-business-phone">
                Business phone
              </FieldLabel>
              <CountryPhoneInput
                id="edit-merchant-business-phone"
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
                  Optional — clear it to remove the number.
                </FieldDescription>
              )}
            </Field>
            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={updateMerchant.isPending}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={!dirty || !name.trim() || updateMerchant.isPending}
              >
                {updateMerchant.isPending ? 'Saving…' : 'Save changes'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
