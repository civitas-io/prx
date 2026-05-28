#!/bin/bash
set -e

mkdir -p models

echo "Downloading model weights..."
curl -L --silent https://huggingface.co/minishlab/potion-code-16M/resolve/main/model.safetensors -o models/potion-retrieval-32M.safetensors

echo "Downloading Model2Vec tokenizer..."
curl -L --silent https://huggingface.co/minishlab/potion-code-16M/resolve/main/tokenizer.json -o models/model2vec_tokenizer.json

echo "Downloading cl100k_base tokenizer..."
curl -L --silent https://huggingface.co/Xenova/gpt-4/resolve/main/tokenizer.json -o models/cl100k_base.json

echo "Converting model to float16..."
python3 -c "
import struct, json
with open('models/potion-retrieval-32M.safetensors', 'rb') as f:
    hs = struct.unpack('<Q', f.read(8))[0]
    hj = f.read(hs)
    header = json.loads(hj)
    data = f.read()
ei = header['embeddings']
s, e = ei['data_offsets']
if ei['dtype'] == 'F32':
    n = (e - s) // 4
    f32 = struct.unpack(f'<{n}f', data[s:e])
    f16 = struct.pack(f'<{n}e', *f32)
    nh = {}
    td = bytearray()
    off = 0
    for nm in ['embeddings', 'mapping', 'weights']:
        info = header[nm]
        os_, oe = info['data_offsets']
        tb = f16 if nm == 'embeddings' else data[os_:oe]
        dt = 'F16' if nm == 'embeddings' else info['dtype']
        nh[nm] = {'dtype': dt, 'shape': info['shape'], 'data_offsets': [off, off + len(tb)]}
        td.extend(tb)
        off += len(tb)
    if '__metadata__' in header: nh['__metadata__'] = header['__metadata__']
    hb = json.dumps(nh).encode()
    while len(hb) % 8: hb += b' '
    with open('models/potion-retrieval-32M.safetensors', 'wb') as f:
        f.write(struct.pack('<Q', len(hb)))
        f.write(hb)
        f.write(bytes(td))
    print('Converted to float16')
else:
    print('Already float16, skipping conversion')
"

echo "Models ready:"
ls -lh models/
