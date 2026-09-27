// Getting a photo ready to store in the database (exposure attachments).
//
// The database lives on OneDrive and is re-synced whole, so a 5 MB phone photo per note
// would bloat it quickly. Anything large is re-encoded as a JPEG no longer than
// MAX_EDGE px on its long side — still plenty to see what was photographed. Small files,
// and formats the webview can't decode (HEIC from a phone, say), are kept byte-for-byte.

const MAX_EDGE = 2000;
const SHRINK_OVER_BYTES = 1_000_000;
const PHOTO_EXT = /\.(jpe?g|png|gif|webp|bmp|heic|heif|tiff?)$/i;

export interface PreparedImage {
  fileName: string;
  mimeType: string;
  dataBase64: string;
}

export function isImageFile(f: File): boolean {
  return f.type.startsWith('image/') || PHOTO_EXT.test(f.name);
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result).replace(/^data:[^,]*,/, ''));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(blob);
  });
}

async function shrink(file: File): Promise<Blob | null> {
  if (file.type === 'image/gif' || file.type === 'image/svg+xml') return null; // animation / vector
  let bmp: ImageBitmap;
  try {
    bmp = await createImageBitmap(file);
  } catch {
    return null; // not decodable here — keep the original
  }
  const scale = Math.min(1, MAX_EDGE / Math.max(bmp.width, bmp.height));
  if (scale === 1 && file.size <= SHRINK_OVER_BYTES) {
    bmp.close();
    return null;
  }
  const canvas = document.createElement('canvas');
  canvas.width = Math.round(bmp.width * scale);
  canvas.height = Math.round(bmp.height * scale);
  canvas.getContext('2d')!.drawImage(bmp, 0, 0, canvas.width, canvas.height);
  bmp.close();
  const out = await new Promise<Blob | null>((res) => canvas.toBlob(res, 'image/jpeg', 0.85));
  // Only worth it if it actually came out smaller.
  return out && out.size < file.size ? out : null;
}

export async function prepareImage(file: File): Promise<PreparedImage> {
  const small = await shrink(file);
  if (small) {
    return {
      fileName: file.name.replace(/\.[^.]+$/, '') + '.jpg',
      mimeType: 'image/jpeg',
      dataBase64: await blobToBase64(small),
    };
  }
  return {
    fileName: file.name || 'image',
    mimeType: file.type || 'application/octet-stream',
    dataBase64: await blobToBase64(file),
  };
}
