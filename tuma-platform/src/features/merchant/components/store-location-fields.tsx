import { ExternalLink } from 'lucide-react'

import {
  Field,
  FieldDescription,
  FieldLabel,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'

/**
 * The location + category fields shared by the store create dialog and the
 * store detail edit form. Coordinates are the tracking contract: every
 * store needs a real, map-verifiable position, because honest distances,
 * ETAs — and one day the rider's route — all start here. The "verify on
 * map" link opens OpenStreetMap at the entered coordinates so the merchant
 * can check the pin before saving.
 */
export function StoreLocationFields({
  category,
  onCategoryChange,
  lat,
  onLatChange,
  lng,
  onLngChange,
}: {
  category: string
  onCategoryChange: (value: string) => void
  lat: string
  onLatChange: (value: string) => void
  lng: string
  onLngChange: (value: string) => void
}) {
  const latNumber = lat.trim() === '' ? null : Number(lat)
  const lngNumber = lng.trim() === '' ? null : Number(lng)
  const latValid =
    latNumber === null || (Number.isFinite(latNumber) && Math.abs(latNumber) <= 90)
  const lngValid =
    lngNumber === null ||
    (Number.isFinite(lngNumber) && Math.abs(lngNumber) <= 180)
  const bothSet = latNumber !== null && lngNumber !== null && latValid && lngValid

  return (
    <>
      <Field>
        <FieldLabel htmlFor="store-category">Category</FieldLabel>
        <Input
          id="store-category"
          autoComplete="off"
          maxLength={50}
          value={category}
          onChange={(e) => onCategoryChange(e.target.value)}
          placeholder="Fast food, Supermarket, Coffee shop…"
        />
        <FieldDescription>
          Shown on the customer&apos;s home screen. One or two words.
        </FieldDescription>
      </Field>
      <Field>
        <FieldLabel>Location coordinates</FieldLabel>
        <div className="grid grid-cols-2 gap-3">
          <div className="grid gap-1.5">
            <Input
              aria-label="Latitude"
              type="number"
              step="any"
              min={-90}
              max={90}
              value={lat}
              onChange={(e) => onLatChange(e.target.value)}
              placeholder="Latitude (−1.9410)"
              className={latValid ? '' : 'border-destructive'}
            />
          </div>
          <div className="grid gap-1.5">
            <Input
              aria-label="Longitude"
              type="number"
              step="any"
              min={-180}
              max={180}
              value={lng}
              onChange={(e) => onLngChange(e.target.value)}
              placeholder="Longitude (30.0912)"
              className={lngValid ? '' : 'border-destructive'}
            />
          </div>
        </div>
        <FieldDescription>
          Where the pickup happens. Real coordinates power real distances,
          ETAs and rider navigation — paste them from a map app (long-press
          your location, copy the numbers).
        </FieldDescription>
        {bothSet ? (
          <a
            href={`https://www.openstreetmap.org/?mlat=${latNumber}&mlon=${lngNumber}#map=17/${latNumber}/${lngNumber}`}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1.5 text-sm text-primary hover:underline"
          >
            <ExternalLink className="size-3.5" aria-hidden />
            Verify this pin on the map
          </a>
        ) : null}
      </Field>
    </>
  )
}
