/**
 * The map pin picker (coverage §7.13): click the map to drop the
 * pickup pin. Uses the existing Google Maps integration
 * (@vis.gl/react-google-maps + VITE_GOOGLE_MAPS_API_KEY). Without a key
 * it degrades honestly — the form explains the pin needs the map, never
 * a fake lat/lng (P2: coordinates are stored, never shown as text).
 */
import { useEffect, useRef } from 'react'
import { APIProvider, Map, useMap, useMapsLibrary } from '@vis.gl/react-google-maps'

import { Icon } from '@/components/ds'

export interface Pin {
  lat: number
  lng: number
}

export function PinPicker({
  pin,
  onPin,
}: {
  pin: Pin | null
  onPin: (pin: Pin | null) => void
}) {
  const apiKey = import.meta.env.VITE_GOOGLE_MAPS_API_KEY as string | undefined

  if (!apiKey) {
    return (
      <div className="flex h-40 flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-line text-center">
        <Icon name="location_off" label="" size={24} className="text-text3" />
        <div className="text-[13px] font-semibold text-text2">
          The map needs a Google Maps key
        </div>
        <div className="max-w-80 text-xs text-text3">
          Set VITE_GOOGLE_MAPS_API_KEY to drop the pin. The store still
          works — riders follow the address text until the pin is set.
        </div>
      </div>
    )
  }

  return (
    <div className="overflow-hidden rounded-xl border border-line">
      <APIProvider apiKey={apiKey}>
        <Map
          center={pin ?? { lat: -1.9441, lng: 30.0619 } /* Kigali */}
          zoom={pin ? 16 : 12}
          gestureHandling="greedy"
          disableDefaultUI
          mapId="tuma-store-pin"
          style={{ width: '100%', height: '280px' }}
          onClick={(e) => {
            if (e.detail.latLng) {
              onPin({
                lat: e.detail.latLng.lat,
                lng: e.detail.latLng.lng,
              })
            }
          }}
        >
          <Marker pin={pin} onPin={onPin} />
        </Map>
      </APIProvider>
      {pin ? (
        <div className="flex items-center gap-2 border-t border-line px-3 py-2">
          <Icon name="location_on" label="" size={16} className="text-success" />
          <span className="flex-1 text-[13px] text-text2">
            Pin dropped — riders navigate here for pickup.
          </span>
          <button
            type="button"
            className="text-[13px] font-semibold text-text2 hover:text-foreground"
            onClick={() => onPin(null)}
          >
            Remove pin
          </button>
        </div>
      ) : (
        <div className="border-t border-line px-3 py-2 text-[13px] text-text3">
          Click the map to drop the pickup pin.
        </div>
      )}
    </div>
  )
}

function Marker({ pin, onPin }: { pin: Pin | null; onPin: (pin: Pin | null) => void }) {
  const map = useMap()
  const markerLib = useMapsLibrary('marker')
  // The AdvancedMarkerElement type lives in the maps marker library's
  // global augmentation, which only loads with the lib; a local any-ref
  // keeps the picker independent of that timing.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const markerRef = useRef<any>(null)
  const onPinRef = useRef(onPin)

  // Keep the callback ref fresh inside an effect, never during render.
  useEffect(() => {
    onPinRef.current = onPin
  }, [onPin])

  useEffect(() => {
    if (!map || !markerLib) return
    if (!markerRef.current) {
      markerRef.current = new markerLib.AdvancedMarkerElement({
        map,
        position: pin,
        gmpDraggable: true,
        title: 'Store pickup point',
      })
      markerRef.current.addListener('dragend', () => {
        const pos = markerRef.current?.position
        if (pos) {
          onPinRef.current({ lat: pos.lat as number, lng: pos.lng as number })
        }
      })
    }
    if (pin) markerRef.current.position = pin
  }, [map, markerLib, pin])

  return null
}
