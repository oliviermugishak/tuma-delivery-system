import { ExternalLink } from 'lucide-react'

import { APIProvider, Map } from '@vis.gl/react-google-maps'

/**
 * The fulfillment sheet's destination pin (tracking doc §4 — where the
 * order goes, NOT tracking): the customer's checkout pin on a small
 * Google map, plus an "open in Google Maps" escape hatch for the
 * operator's phone. The key is the web-restricted one, injected at build
 * time (VITE_GOOGLE_MAPS_API_KEY in tuma-platform/.env, gitignored).
 * Without a key the map degrades honestly to the coordinates link —
 * never a broken map or a guessed geocode (a pin-less order shows the
 * honest text only).
 */
export function DeliveryPinCard({
  lat,
  lng,
}: {
  lat: number | null | undefined
  lng: number | null | undefined
}) {
  const apiKey = import.meta.env.VITE_GOOGLE_MAPS_API_KEY as
    | string
    | undefined
  const hasPin = lat != null && lng != null

  const gmapsLink = hasPin
    ? `https://www.google.com/maps/search/?api=1&query=${lat},${lng}`
    : null

  return (
    <div className="mt-2 grid gap-2">
      {hasPin && apiKey ? (
        <div className="overflow-hidden rounded-xl border">
          <APIProvider apiKey={apiKey}>
            <Map
              center={{ lat: lat as number, lng: lng as number }}
              zoom={16}
              disableDefaultUI
              gestureHandling={'greedy'}
              style={{ width: '100%', height: '180px' }}
              mapId="tuma-fulfillment-pin"
            />
          </APIProvider>
        </div>
      ) : null}
      {hasPin ? (
        <a
          href={gmapsLink as string}
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-1.5 text-xs text-primary hover:underline"
        >
          <ExternalLink className="size-3" aria-hidden />
          Open the delivery pin in Google Maps
          {apiKey ? '' : ` — (${lat}, ${lng})`}
        </a>
      ) : (
        <p className="text-xs text-muted-foreground">
          No delivery pin on this order — the address text is all the
          customer gave.
        </p>
      )}
    </div>
  )
}
