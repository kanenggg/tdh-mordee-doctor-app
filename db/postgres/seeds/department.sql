INSERT INTO public.department (department_id, name, counseling_areas, updated_at)
VALUES (205, '{
  "en": "Kid",
  "th": "เด็ก"
}'::jsonb,
        '[
          {
            "th": "ตรวจและรักษาโรคทั่วไปในเด็ก",
            "en": "Treatment of common childhood illnesses"
          },
          {
            "th": "วัคซีนและการติดตามพัฒนาการ",
            "en": "Vaccination and developmental monitoring"
          },
          {
            "th": "ให้คำแนะนำเรื่องโภชนาการเด็ก",
            "en": "Pediatric nutrition counseling"
          },
          {
            "th": "ดูแลเด็กที่มีภูมิแพ้และปัญหาการนอน",
            "en": "Allergy and sleep problem management"
          }
        ]'::jsonb, now()),
       (206, '{
         "en": "Women''s Health",
         "th": "สุขภาพผู้หญิง"
       }'::jsonb, '[
         {
           "th": "ให้คำแนะนำสุขอนามัยสตรีและการดูแลตนเอง",
           "en": "Female hygiene and self-care guidance"
         },
         {
           "th": "ดูแลปัญหาประจำเดือนผิดปกติและอาการปวดประจำเดือน",
           "en": "Management of irregular periods and menstrual pain"
         },
         {
           "th": "การตรวจดูแลอาการและความผิดปกติของฮอร์โมน",
           "en": "Menopause and hormonal balance care"
         },
         {
           "th": "รักษาภาวะติดเชื้อในช่องคลอดและอาการตกขาว",
           "en": "Treatment of vaginal infections and discharge"
         }
       ]'::jsonb, now()),
       (207, '{
         "en": "Beauty&Anti-aging",
         "th": "ความงาม&ชะลอ​วัย "
       }'::jsonb, '[
         {
           "th": "วางแผนโภชนาการและการออกกำลังกายเพื่อปรับไลฟ์สไตล์ให้สุขภาพดีขึ้น",
           "en": "Nutrition and exercise planning to improve lifestyle and overall health"
         },
         {
           "th": "ให้คำปรึกษาเรื่องริ้วรอยและการดูแลผิว",
           "en": "Consultation on wrinkle care and skincare"
         },
         {
           "th": "วางแผนการดูแลสุขภาพเพื่อชะลอวัย",
           "en": "Personalized health planning for anti-aging and longevity"
         }
       ]'::jsonb, now()),
       (208, '{
         "en": "Mental Health",
         "th": "สุขภาพใจ"
       }'::jsonb, '[
         {
           "th": "ให้คำปรึกษาเรื่องความเครียดและความสัมพันธ์",
           "en": "Counseling for stress and relationship issues"
         },
         {
           "th": "การจัดการอารมณ์และสมาธิ",
           "en": "Emotional management and mindfulness"
         },
         {
           "th": "การสร้างสมดุลชีวิตและการดูแลตนเอง",
           "en": "Life balance and self-care guidance"
         },
         {
           "th": "การพัฒนาแนวคิดเชิงบวกและสุขภาวะทางใจ",
           "en": "Positive thinking and mental resilience support"
         }
       ]'::jsonb, now()),
       (209, '{
         "en": "Internal Medicine",
         "th": "อายุรกรรม"
       }'::jsonb, '[
         {
           "th": "ตรวจและรักษาเบาหวาน ความดันโลหิตสูง และไขมันในเลือดสูง",
           "en": "Treatment of diabetes, hypertension, and high cholesterol"
         },
         {
           "th": "ดูแลโรคหัวใจ ปอด ทางเดินอาหาร และระบบเมตาบอลิก",
           "en": "Care for cardiovascular, respiratory, and digestive disorders"
         },
         {
           "th": "ติดตามและจัดการโรคเรื้อรังระยะยาว",
           "en": "Long-term management of chronic diseases"
         },
         {
           "th": "ตรวจคัดกรองโรคตับ ไต และต่อมไร้ท่อ",
           "en": "Screening for liver, kidney, and endocrine disorders"
         },
         {
           "th": "ประเมินความเสี่ยงโรคหลอดเลือดและโรคเรื้อรัง",
           "en": "Cardiovascular and chronic disease risk assessment"
         },
         {
           "th": "ให้คำแนะนำการใช้ยาและการควบคุมอาหาร",
           "en": "Medication and dietary counseling"
         },
         {
           "th": "ดูแลผู้สูงอายุและผู้มีโรคประจำตัวหลายโรค",
           "en": "Geriatric and multi-comorbidity care"
         }
       ]'::jsonb, now()),
       (210, '{
         "en": "General Health",
         "th": "โรคทั่วไป"
       }'::jsonb, '[
         {
           "th": "ตรวจและรักษาโรคทั่วไป เช่น ไข้ ไอ เจ็บคอ ปวดท้อง",
           "en": "Diagnosis and treatment of common illnesses (fever, cough, sore throat, stomach pain)"
         },
         {
           "th": "ให้คำแนะนำการใช้ยาที่ถูกต้องและปลอดภัย",
           "en": "Safe and appropriate medication guidance"
         },
         {
           "th": "ตรวจและรักษาโรคเรื้อรังเบื้องต้น",
           "en": "Basic screening for chronic conditions"
         },
         {
           "th": "ประเมินอาการและส่งต่อผู้เชี่ยวชาญเมื่อจำเป็น",
           "en": "Initial assessment and referral to specialists if needed"
         },
         {
           "th": "ให้คำปรึกษาอาการเจ็บป่วยเฉียบพลันในชีวิตประจำวัน",
           "en": "Consultation for everyday acute symptoms"
         }
       ]'::jsonb, now()),
       (211, '{
         "en": "Ear Nose Throat",
         "th": "หู คอ จมูก"
       }'::jsonb, '[
         {
           "th": "รักษาโรคภูมิแพ้ หวัดเรื้อรัง และไซนัสอักเสบ",
           "en": "Treatment of allergy, chronic rhinitis, and sinusitis"
         },
         {
           "th": "ตรวจการได้ยินและปัญหาหูอื้อ",
           "en": "Hearing test and tinnitus management"
         },
         {
           "th": "รักษาอาการเจ็บคอ กลืนลำบาก เสียงแหบ",
           "en": "Throat pain, hoarseness, and swallowing disorder care"
         }
       ]'::jsonb, now()),
       (212, '{
         "en": "Men''s Health",
         "th": "สุขภาพเพศชาย"
       }'::jsonb, '[
         {
           "th": "ตรวจและประเมินสุขภาพเพศชายโดยรวม",
           "en": "Comprehensive men’s health assessment"
         },
         {
           "th": "ดูแลภาวะหย่อนสมรรถภาพทางเพศ",
           "en": "Management of erectile dysfunction"
         },
         {
           "th": "รักษาภาวะฮอร์โมนเพศชายต่ำ",
           "en": "Treatment for low testosterone conditions"
         },
         {
           "th": "ให้คำปรึกษาปัญหาการหลั่งเร็วหรือหลั่งล่าช้า",
           "en": "Counseling for premature or delayed ejaculation"
         },
         {
           "th": "ตรวจและประเมินภาวะมีบุตรยากในผู้ชาย",
           "en": "Evaluation of male infertility"
         },
         {
           "th": "ตรวจต่อมลูกหมากและคัดกรองความเสี่ยงโรค",
           "en": "Prostate examination and risk screening"
         },
         {
           "th": "ให้คำปรึกษาด้านสุขภาพทางเพศและการป้องกันโรคติดต่อ",
           "en": "Sexual health counseling and STI prevention"
         }
       ]'::jsonb, now()),
       (213, '{
         "en": "Office Syndrome",
         "th": "ออฟฟิศซินโดรม"
       }'::jsonb, '[
         {
           "th": "รักษาอาการปวดคอ บ่า ไหล่ หลัง จากการทำงาน",
           "en": "Treatment of neck, shoulder, and back pain from work posture"
         },
         {
           "th": "ปรับท่าทางและให้คำแนะนำการยืดกล้ามเนื้อ",
           "en": "Posture correction and stretching guidance"
         },
         {
           "th": "กายภาพบำบัดและฟื้นฟูสมรรถภาพ",
           "en": "Physical therapy and rehabilitation"
         }
       ]'::jsonb, now()),
       (214, '{
         "en": "Surgery",
         "th": "ศัลยกรรมและการผ่าตัด"
       }'::jsonb, '[
         {
           "th": "ดูแลแผลผ่าตัดและแผลติดเชื้อ",
           "en": "Post-surgical wound care"
         },
         {
           "th": "ให้คำแนะนำก่อนและหลังผ่าตัด",
           "en": "Pre- and post-operative counseling"
         }
       ]'::jsonb, now()),
       (215, '{
         "en": "Pharmacy",
         "th": "เภสัชกรรม"
       }'::jsonb, '[
         {
           "th": "ให้คำแนะนำการใช้ยาอย่างถูกต้องและปลอดภัย",
           "en": "Safe and appropriate medication counseling"
         },
         {
           "th": "ตรวจสอบการแพ้ยาและการใช้ยาซ้ำซ้อน",
           "en": "Allergy and drug interaction checks"
         },
         {
           "th": "ให้คำแนะนำการเก็บรักษายา",
           "en": "Guidance on proper medication storage"
         },
         {
           "th": "ให้ข้อมูลยาทางเลือกและสมุนไพร",
           "en": "Information on alternative and herbal medicines"
         }
       ]'::jsonb, now()),
       (216, '{
         "en": "Oncology",
         "th": "มะเร็ง"
       }'::jsonb, '[
         {
           "th": "ทำความเข้าใจผลตรวจ ชิ้นเนื้อ ระยะโรค และแผนการรักษา",
           "en": "Understanding test results, biopsy findings, staging, and treatment plans"
         },
         {
           "th": "อาการข้างเคียงจากเคมีบำบัด ฉายแสง และยามุ่งเป้า",
           "en": "Side effects from chemotherapy, radiation, and targeted therapy"
         },
         {
           "th": "การดูแลประคับประคอง โภชนาการ และคุณภาพชีวิต",
           "en": "Supportive care, nutrition, and quality-of-life concerns"
         }
       ]'::jsonb, now()),
       (217, '{
         "en": "Bones",
         "th": "กระดูกและข้อ"
       }'::jsonb, '[
         {
           "th": "รักษาอาการปวดหลัง ปวดเข่า ปวดไหล่",
           "en": "Treatment of back, knee, and shoulder pain"
         },
         {
           "th": "ตรวจภาวะกระดูกพรุนและข้อเสื่อม",
           "en": "Osteoporosis and joint degeneration management"
         },
         {
           "th": "ฟื้นฟูการเคลื่อนไหวหลังบาดเจ็บ",
           "en": "Post-injury rehabilitation"
         },
         {
           "th": "ให้คำแนะนำการออกกำลังกายที่เหมาะสมหลังการบาดเจ็บ",
           "en": "Exercise and movement guidance"
         }
       ]'::jsonb, now()),
       (218, '{
         "en": "Chinese Medicine",
         "th": "แพทย์แผนจีน"
       }'::jsonb, '[
         {
           "th": "ฝังเข็มสำหรับปวดเรื้อรัง ไมเกรน และออฟฟิศซินโดรม",
           "en": "Acupuncture for chronic pain, migraine, and office syndrome"
         },
         {
           "th": "ปรับสมดุลร่างกาย นอนหลับยาก และความเครียด",
           "en": "Body balance, sleep difficulty, and stress support"
         },
         {
           "th": "สมุนไพรจีน การดูแลตนเอง และข้อควรระวังร่วมกับยาเดิม",
           "en": "Chinese herbs, self-care, and precautions with current medicines"
         }
       ]'::jsonb, now()),
       (219, '{
         "en": "Thai Traditional Medicine",
         "th": "แพทย์แผนไทย"
       }'::jsonb, '[
         {
           "th": "นวดไทย ประคบสมุนไพร และดูแลอาการปวดเมื่อย",
           "en": "Thai massage, herbal compress, and muscle ache care"
         },
         {
           "th": "สมุนไพรไทยสำหรับอาการทั่วไปและข้อควรระวัง",
           "en": "Thai herbs for common symptoms and safety precautions"
         },
         {
           "th": "ฟื้นฟูร่างกายหลังเจ็บป่วยและดูแลสุขภาพตามธาตุเจ้าเรือน",
           "en": "Recovery after illness and traditional constitution-based self-care"
         }
       ]'::jsonb, now()),
       (220, '{
         "en": "Eyes",
         "th": "ตา"
       }'::jsonb, '[
         {
           "th": "ตรวจและรักษาโรคตาทั่วไป",
           "en": "Diagnosis and treatment of common eye diseases"
         },
         {
           "th": "รักษาภาวะตาแห้ง เยื่อบุตาอักเสบ",
           "en": "Treatment of dry eyes and conjunctivitis"
         },
         {
           "th": "ภาวะต้อกระจกและต้อหิน",
           "en": "Cataract and glaucoma evaluation"
         }
       ]'::jsonb, now()),
       (221, '{
         "en": "Dental",
         "th": "ทันตกรรม"
       }'::jsonb, '[
         {
           "th": "ให้คำปรึกษาสุขภาพช่องปาก",
           "en": "Oral health consultation"
         },
         {
           "th": "ปวดฟัน ปวดเหงือก แผลในปาก เหงือกบวม",
           "en": "Toothache, gum pain, mouth ulcers, and swollen gums"
         }
       ]'::jsonb, now()),
       (222, '{
         "en": "Nutrition",
         "th": "โภชนาการ"
       }'::jsonb, '[
         {
           "th": "ประเมินภาวะโภชนาการและวิเคราะห์พฤติกรรมการกิน",
           "en": "Nutritional assessment and eating behavior analysis"
         },
         {
           "th": "วางแผนโภชนาการเฉพาะบุคคลสำหรับควบคุมน้ำหนัก",
           "en": "Personalized diet planning for weight management"
         },
         {
           "th": "ให้คำปรึกษาอาหารสำหรับโรคเบาหวาน ความดัน และไขมันสูง",
           "en": "Dietary counseling for diabetes, hypertension, and high cholesterol"
         },
         {
           "th": "ให้คำแนะนำการเลือกอาหารและปรับพฤติกรรมการกิน",
           "en": "Guidance on healthy food choices and eating habits"
         },
         {
           "th": "ให้คำปรึกษาโภชนาการสำหรับผู้สูงอายุ",
           "en": "Nutritional advice for older adults"
         },
         {
           "th": "วางแผนอาหารสำหรับผู้ที่ออกกำลังกายและนักกีฬา",
           "en": "Diet planning for active individuals and athletes"
         }
       ]'::jsonb, now()),
       (268, '{
         "en": "Psychiatry",
         "th": "จิตเวช"
       }'::jsonb, '[
         {
           "th": "ให้คำปรึกษาเรื่องความเครียด วิตกกังวล ซึมเศร้า",
           "en": "Counseling for stress, anxiety, and depression"
         },
         {
           "th": "รักษาโรคนอนไม่หลับ",
           "en": "Treatment of insomnia and sleep disorders"
         },
         {
           "th": "ดูแลผู้ป่วยที่มีปัญหาสุขภาพจิตเรื้อรัง",
           "en": "Management of chronic mental health conditions"
         },
         {
           "th": "ให้คำแนะนำด้านจิตบำบัดและการใช้ยา",
           "en": "Psychotherapy and medication management"
         }
       ]'::jsonb, now())
ON CONFLICT (department_id) DO UPDATE
    SET name             = EXCLUDED.name,
        counseling_areas = EXCLUDED.counseling_areas,
        updated_at       = now();

INSERT INTO public.department(department_id, name, counseling_areas, created_at)
VALUES (225, '{
  "en": "Skin Disease",
  "th": "โรคผิวหนัง"
}'::jsonb,
        '[
          {
            "th": "ตรวจและรักษาโรคผิวหนังทั่วไป",
            "en": "Diagnosis and treatment of common skin diseases"
          },
          {
            "th": "ผื่นคัน สะเก็ดเงิน งูสวัด",
            "en": "Rash, psoriasis, and shingles care"
          },
          {
            "th": "ผิวแพ้ง่าย ผิวภูมิแพ้",
            "en": "Sensitive skin and allergic skin condition management"
          }
        ]'::jsonb, now()),
       (227, '{
         "en": "Physical Therapy",
         "th": "กายภาพบำบัด"
       }'::jsonb, '[
         {
           "th": "",
           "en": ""
         }
       ]'::jsonb, now()),
       (228, '{
         "en": "Infectious Diseases",
         "th": "โรคติดเชื้อ"
       }'::jsonb, '[
         {
           "th": "ตรวจและติดตามโรคไวรัสตับอักเสบ เอดส์ และโรคติดต่อทางเพศสัมพันธ์",
           "en": "Screening and management of hepatitis, HIV, and sexually transmitted diseases"
         },
         {
           "th": "ประเมินความเสี่ยงการติดเชื้อหลังสัมผัสเชื้อหรือการเดินทาง",
           "en": "Risk assessment after exposure or travel-related infections"
         },
         {
           "th": "ให้คำแนะนำเรื่องวัคซีนและการป้องกันโรคติดเชื้อ",
           "en": "Vaccination and infectious disease prevention counseling"
         },
         {
           "th": "ให้คำปรึกษาเรื่องการใช้ยาปฏิชีวนะอย่างเหมาะสม",
           "en": "Guidance on appropriate antibiotic use"
         },
         {
           "th": "ให้คำแนะนำการป้องกันการแพร่เชื้อในครอบครัวหรือที่ทำงาน",
           "en": "Infection control advice for families and workplaces"
         }
       ]'::jsonb, now())
ON CONFLICT (department_id) DO UPDATE
    SET name             = EXCLUDED.name,
        counseling_areas = EXCLUDED.counseling_areas,
        updated_at       = now();
