#include <stdint.h>
#include <stdio.h>

#include "silk_codec.h"

int main(void) {
  uint8_t pcm[960] = {0};
  SilkCodecBuffer encoded = {0};
  SilkCodecBuffer decoded = {0};

  SilkCodecStatus encode_status =
      silk_codec_encode(pcm, sizeof(pcm), 24000, 24000, false, &encoded);
  if (encode_status != Success) {
    fprintf(stderr, "encode failed: %d\n", encode_status);
    return 1;
  }

  SilkCodecStatus decode_status =
      silk_codec_decode(encoded.ptr, encoded.len, 24000, &decoded);
  if (decode_status != Success) {
    fprintf(stderr, "decode failed: %d\n", decode_status);
    silk_codec_free_buffer(encoded.ptr, encoded.len);
    return 1;
  }

  printf("encoded=%zu decoded=%zu\n", encoded.len, decoded.len);

  silk_codec_free_buffer(encoded.ptr, encoded.len);
  silk_codec_free_buffer(decoded.ptr, decoded.len);
  return 0;
}
