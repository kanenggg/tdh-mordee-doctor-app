INSERT INTO public.clinics(clinic_id, name)
VALUES (1, '{"en": "Vaccine Point", "th": "วัคซีนพอยท์"}'),
       (2, '{"en": "คลินิกฟังผลเลือด", "th": "คลินิกฟังผลเลือด"}'),
       (3, '{"en": "คลินิกเติมยา NCDs", "th": "คลินิกเติมยา NCDs"}'),
       (4, '{"en": "KKP สุขใจคลินิก", "th": "KKP สุขใจคลินิก"}')
ON CONFLICT (clinic_id) DO UPDATE SET
    name       = EXCLUDED.name,
    updated_at = now();
