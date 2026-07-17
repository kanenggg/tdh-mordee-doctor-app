-- Department examples for local/dev ranking doctor detail responses.
-- counseling_areas is localized free text so it can be edited by a WYSIWYG later.
INSERT INTO department (department_id, name, counseling_areas, updated_at)
VALUES
  (205, '{"en": "Kid", "th": "เด็ก"}'::jsonb, '{
    "th": "ไข้ ไอ น้ำมูก และหวัดในเด็ก\nผื่นแพ้ ผิวหนังอักเสบ และภูมิแพ้\nพัฒนาการ การกิน และการนอนของเด็ก",
    "en": "Fever, cough, runny nose, and common childhood colds\nRashes, eczema, and allergies\nChild development, feeding, and sleep concerns"
  }'::jsonb, now()),
  (206, '{"en": "Women''s health", "th": "สุขภาพผู้หญิง"}'::jsonb, '{
    "th": "ประจำเดือนผิดปกติ ปวดประจำเดือน และตกขาว\nการคุมกำเนิด วางแผนครอบครัว และเตรียมตั้งครรภ์\nอาการวัยทอง ฮอร์โมน และสุขภาพเต้านม",
    "en": "Irregular periods, menstrual pain, and vaginal discharge\nContraception, family planning, and pregnancy preparation\nMenopause symptoms, hormones, and breast health"
  }'::jsonb, now()),
  (207, '{"en": "Beauty&Anti-aging", "th": "ความงาม&ชะลอ​วัย "}'::jsonb, '{
    "th": "สิว ฝ้า กระ จุดด่างดำ และรอยแผลเป็น\nริ้วรอย ผิวหย่อนคล้อย และการดูแลผิวตามวัย\nผมร่วง เล็บเปราะ และการเลือกสกินแคร์ให้เหมาะกับผิว",
    "en": "Acne, melasma, dark spots, and scars\nWrinkles, skin laxity, and age-appropriate skin care\nHair loss, brittle nails, and choosing skin care for your skin type"
  }'::jsonb, now()),
  (208, '{"en": "Mental Health", "th": "สุขภาพใจ"}'::jsonb, '{
    "th": "ความเครียด วิตกกังวล และภาวะหมดไฟ\nนอนไม่หลับ พักผ่อนไม่เพียงพอ และอารมณ์แปรปรวน\nความสัมพันธ์ ครอบครัว และการปรับตัวในชีวิตประจำวัน",
    "en": "Stress, anxiety, and burnout\nInsomnia, poor sleep, and mood changes\nRelationships, family concerns, and day-to-day adjustment"
  }'::jsonb, now()),
  (209, '{"en": "Internal Medicine", "th": "อายุรกรรม"}'::jsonb, '{
    "th": "เบาหวาน ความดัน ไขมัน และโรคเรื้อรัง\nกรดไหลย้อน ปวดท้อง ท้องผูก และลำไส้แปรปรวน\nเวียนศีรษะ อ่อนเพลีย ใจสั่น และตรวจผลเลือด",
    "en": "Diabetes, hypertension, cholesterol, and chronic disease follow-up\nReflux, abdominal pain, constipation, and irritable bowel symptoms\nDizziness, fatigue, palpitations, and lab result review"
  }'::jsonb, now()),
  (210, '{"en": "General Health", "th": "โรคทั่วไป"}'::jsonb, '{
    "th": "ไข้ ไอ เจ็บคอ น้ำมูก และอาการหวัด\nปวดหัว ปวดเมื่อย คลื่นไส้ ท้องเสีย และอาหารเป็นพิษ\nปรึกษาผลตรวจสุขภาพ วัคซีน และการดูแลตัวเองเบื้องต้น",
    "en": "Fever, cough, sore throat, runny nose, and cold symptoms\nHeadache, body aches, nausea, diarrhea, and food poisoning\nHealth check results, vaccines, and basic self-care advice"
  }'::jsonb, now()),
  (211, '{"en": "Ear Nose Throat", "th": "หู คอ จมูก"}'::jsonb, '{
    "th": "ภูมิแพ้ คัดจมูก น้ำมูกไหล และไซนัสอักเสบ\nเจ็บคอ เสียงแหบ กลืนเจ็บ และต่อมทอนซิลอักเสบ\nหูอื้อ ปวดหู บ้านหมุน และปัญหาการได้ยิน",
    "en": "Allergies, nasal congestion, runny nose, and sinusitis\nSore throat, hoarseness, painful swallowing, and tonsillitis\nBlocked ears, ear pain, vertigo, and hearing concerns"
  }'::jsonb, now()),
  (212, '{"en": "LGBTQ", "th": "LGBTQ"}'::jsonb, '{
    "th": "สุขภาพทางเพศ การตรวจคัดกรอง และการป้องกันโรคติดต่อทางเพศสัมพันธ์\nการใช้ฮอร์โมนอย่างปลอดภัยและการติดตามผลตรวจ\nสุขภาพใจ ความสัมพันธ์ และการยอมรับตัวตน",
    "en": "Sexual health, screening, and STI prevention\nSafe hormone use and follow-up lab monitoring\nMental health, relationships, and identity support"
  }'::jsonb, now()),
  (213, '{"en": "Office Syndrome", "th": "ออฟฟิศซินโดรม"}'::jsonb, '{
    "th": "ปวดคอ บ่า ไหล่ หลัง และสะบักจากการทำงาน\nชามือ ปวดข้อมือ นิ้วล็อก และเอ็นอักเสบ\nปรับท่านั่ง ยืดเหยียด และป้องกันอาการกำเริบ",
    "en": "Work-related neck, shoulder, upper back, and back pain\nHand numbness, wrist pain, trigger finger, and tendon irritation\nPosture, stretching, and preventing symptom flare-ups"
  }'::jsonb, now()),
  (214, '{"en": "Surgery", "th": "ศัลยกรรมและการผ่าตัด"}'::jsonb, '{
    "th": "แผล ฝี ก้อน ซีสต์ และการดูแลแผลหลังหัตถการ\nริดสีดวง ไส้เลื่อน นิ่ว และอาการที่อาจต้องผ่าตัด\nประเมินอาการก่อนพบศัลยแพทย์และคำแนะนำหลังผ่าตัด",
    "en": "Wounds, abscesses, lumps, cysts, and post-procedure wound care\nHemorrhoids, hernias, stones, and symptoms that may need surgery\nPre-surgical symptom review and post-operative guidance"
  }'::jsonb, now()),
  (215, '{"en": "Pharmacy", "th": "เภสัชกรรม"}'::jsonb, '{
    "th": "วิธีใช้ยา ขนาดยา เวลาใช้ยา และการลืมกินยา\nอาการข้างเคียง แพ้ยา และยาที่อาจตีกัน\nการเลือกยาสามัญประจำบ้านและอาหารเสริมอย่างเหมาะสม",
    "en": "How to take medicines, dosing, timing, and missed doses\nSide effects, drug allergies, and possible drug interactions\nChoosing over-the-counter medicines and supplements appropriately"
  }'::jsonb, now()),
  (216, '{"en": "Oncology", "th": "มะเร็ง"}'::jsonb, '{
    "th": "ทำความเข้าใจผลตรวจ ชิ้นเนื้อ ระยะโรค และแผนการรักษา\nอาการข้างเคียงจากเคมีบำบัด ฉายแสง และยามุ่งเป้า\nการดูแลประคับประคอง โภชนาการ และคุณภาพชีวิต",
    "en": "Understanding test results, biopsy findings, staging, and treatment plans\nSide effects from chemotherapy, radiation, and targeted therapy\nSupportive care, nutrition, and quality-of-life concerns"
  }'::jsonb, now()),
  (217, '{"en": "Bones", "th": "กระดูกและข้อ"}'::jsonb, '{
    "th": "ปวดเข่า ปวดไหล่ ปวดหลัง และข้อเสื่อม\nข้อเท้าแพลง กล้ามเนื้ออักเสบ เอ็นอักเสบ และบาดเจ็บจากกีฬา\nกระดูกพรุน กระดูกหัก และการฟื้นฟูหลังบาดเจ็บ",
    "en": "Knee, shoulder, back pain, and osteoarthritis\nAnkle sprains, muscle strain, tendonitis, and sports injuries\nOsteoporosis, fractures, and recovery after injury"
  }'::jsonb, now()),
  (218, '{"en": "Chinese Medicine", "th": "แพทย์แผนจีน"}'::jsonb, '{
    "th": "ฝังเข็มสำหรับปวดเรื้อรัง ไมเกรน และออฟฟิศซินโดรม\nปรับสมดุลร่างกาย นอนหลับยาก และความเครียด\nสมุนไพรจีน การดูแลตนเอง และข้อควรระวังร่วมกับยาเดิม",
    "en": "Acupuncture for chronic pain, migraine, and office syndrome\nBody balance, sleep difficulty, and stress support\nChinese herbs, self-care, and precautions with current medicines"
  }'::jsonb, now()),
  (219, '{"en": "Thai Traditional Medicine", "th": "แพทย์แผนไทย"}'::jsonb, '{
    "th": "นวดไทย ประคบสมุนไพร และดูแลอาการปวดเมื่อย\nสมุนไพรไทยสำหรับอาการทั่วไปและข้อควรระวัง\nฟื้นฟูร่างกายหลังเจ็บป่วยและดูแลสุขภาพตามธาตุเจ้าเรือน",
    "en": "Thai massage, herbal compress, and muscle ache care\nThai herbs for common symptoms and safety precautions\nRecovery after illness and traditional constitution-based self-care"
  }'::jsonb, now()),
  (220, '{"en": "Eyes", "th": "ตา"}'::jsonb, '{
    "th": "ตาแดง คันตา ตาแห้ง และภูมิแพ้ตา\nปวดตา มองไม่ชัด เห็นแสงวาบ และอาการที่ควรรีบตรวจ\nการใช้คอนแทคเลนส์ ยาหยอดตา และดูแลดวงตาจากหน้าจอ",
    "en": "Red eyes, itchy eyes, dry eyes, and eye allergies\nEye pain, blurry vision, flashing lights, and urgent warning signs\nContact lens use, eye drops, and screen-related eye care"
  }'::jsonb, now()),
  (221, '{"en": "Dental", "th": "ทันตกรรม"}'::jsonb, '{
    "th": "ปวดฟัน เสียวฟัน เหงือกบวม และเลือดออกตามไรฟัน\nฟันผุ กลิ่นปาก แผลในปาก และปัญหาฟันคุด\nดูแลช่องปาก จัดฟัน ฟอกสีฟัน และติดตามหลังทำฟัน",
    "en": "Toothache, tooth sensitivity, swollen gums, and gum bleeding\nCavities, bad breath, mouth ulcers, and wisdom tooth concerns\nOral care, braces, whitening, and follow-up after dental treatment"
  }'::jsonb, now()),
  (222, '{"en": "Nutrition", "th": "โภชนาการ"}'::jsonb, '{
    "th": "ควบคุมน้ำหนัก ลดไขมัน และเพิ่มกล้ามเนื้ออย่างเหมาะสม\nอาหารสำหรับเบาหวาน ความดัน ไขมัน และโรคไต\nวางแผนมื้ออาหาร อ่านฉลากโภชนาการ และเลือกอาหารเสริม",
    "en": "Weight management, fat loss, and healthy muscle gain\nNutrition for diabetes, hypertension, cholesterol, and kidney disease\nMeal planning, nutrition labels, and supplement choices"
  }'::jsonb, now()),
  (268, '{"en": "Psychiatry", "th": "จิดเวช"}'::jsonb, '{
    "th": "ซึมเศร้า วิตกกังวล แพนิค และอารมณ์แปรปรวน\nนอนไม่หลับ สมาธิสั้น และปัญหาการใช้สารเสพติด\nติดตามยา ประเมินอาการ และวางแผนการดูแลต่อเนื่อง",
    "en": "Depression, anxiety, panic symptoms, and mood instability\nInsomnia, attention difficulties, and substance use concerns\nMedication follow-up, symptom review, and ongoing care planning"
  }'::jsonb, now())
ON CONFLICT (department_id) DO UPDATE
SET name = EXCLUDED.name,
    counseling_areas = EXCLUDED.counseling_areas,
    updated_at = now();
