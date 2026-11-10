# Research

Done 2026-11-10 with web searches. Confidence: H (primary source read or several agreeing sources), M (secondary source, or summary only), L (single weak source). Most rule findings come from law-firm and trade summaries, not the rule texts themselves; the key ones are checked again before they go in the README.

## 1. Lab reports: the problem

| Finding | Source | Confidence |
|---|---|---|
| Since April 2021 the US information-blocking rule requires providers to make nearly all test results available to patients without delay, so patients often see raw results before a clinician explains them | Summary of the rule in coverage of Steitz et al., JAMA Netw Open 2023: [UT Southwestern, 2023-03](https://www.utsouthwestern.edu/newsroom/articles/year-2023/march-medical-test-results-online.html) | M |
| Of 8,139 patients surveyed, about 96% prefer immediate access; 7.5% felt more worried reading results before talking to a clinician, rising to 16.5% when a result was abnormal | Steitz et al., JAMA Netw Open 2023, via [News-Medical, 2023-03-20](https://www.news-medical.net/news/20230320/Study-Patients-overwhelmingly-prefer-to-receive-test-results-immediately-from-online-medical-portals.aspx) | M |
| 37.2% of patients refreshed their portal while waiting for results | A separate JAMA Netw Open study, via [TechTarget](https://www.techtarget.com/patientengagement/news/366622613/Are-patients-stressed-awaiting-patient-portal-test-results) | M |
| Language models did "very poorly" at classifying lab results against reference ranges they were given | Hager et al., Nature Medicine 2024, via [HTN, 2024-07-12](https://htn.co.uk/2024/07/12/study-highlights-limitations-of-large-language-models-around-clinical-diagnosis-interpretation-of-results-and-integration) | M |
| Model answers about reference intervals are inconsistent between runs; the authors recommend giving the intervals in the prompt | [Frontiers in AI, 2025](https://public-pages-files-2025.frontiersin.org/journals/artificial-intelligence/articles/10.3389/frai.2025.1681979/epub) | M |
| A tightly prompted Claude chatbot explained a tested set of blood chemistry results with no hallucinations (one narrow study) | [MDPI Applied Sciences 15(8):4232, 2025-04](https://www.mdpi.com/2076-3417/15/8/4232) | L |
| In a neighbouring task, GPT-4 summaries of 100 emergency visits: 42% had hallucinations, 47% omissions | [medRxiv 2024.04.03.24305088](https://www.medrxiv.org/content/10.1101/2024.04.03.24305088v1.full.txt) | M |
| In patient-message drafts (also neighbouring), 7.1% of unedited GPT-4 drafts risked severe harm, mostly by not telling the patient to seek urgent care | [Lancet Digital Health 2024, PMC11829255](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC11829255/) | H |

**What this means for the design (recommendation):** patients want their results quickly and are sometimes worried by them; models explain results reasonably when given the exact values and ranges, but get numbers, ranges and omissions wrong often enough that a draft must be checked in code and approved by a physician. Wrong numbers, invented ranges and missing results are the failures to measure.

## 2. Rules that touch a lab report assistant

| Rule | What it means here | Source | Confidence |
|---|---|---|---|
| FDA clinical decision support guidance, revised 2026-01-06 | Clinician-facing software whose output a clinician can independently review stays outside device rules more easily. A report the physician reviews and approves fits that; anything that diagnoses or ranks urgency moves toward a device | [Covington, 2026-01](https://www.cov.com/en/news-and-insights/insights/2026/01/5-key-takeaways-from-fdas-revised-clinical-decision-support-cds-software-guidance) | M |
| California AB 3030, in force 2025-01-01 | AI-written clinical messages to patients need a disclaimer, unless a licensed provider reads and reviews them first. Physician approval is the design, and the report still says how it was made | [AFS Law](https://www.afslaw.com/perspectives/alerts/california-requires-disclaimers-health-care-providers-ai-generated-patient) | H |
| Texas TRAIGA / SB 1188, in force 2026-01-01 | Practitioners must disclose AI use in diagnosis or treatment; sources disagree on scope | [Akerman](https://www.akerman.com/en/perspectives/hrx-new-year-new-ai-rules-healthcare-ai-laws-now-in-effect.html) | M |
| HIPAA Security Rule audit controls (45 CFR 164.312(b)) and activity review (164.308(a)(1)(ii)(D)) | Record and review who did what with patient data: the audit log | Wording from memory, unverified; the proposed overhaul is reportedly delayed to July 2027 ([Coalfire](https://coalfire.com/the-coalfire-blog/the-hipaa-security-rule-update-is-delayed-to-july-2027-a-new-opportunity-to-strengthen-your-compliance-roadmap)) | M |
| ONC HTI-5 final rule, published 2026-10-28, effective 2026-12-28 | Removed 14 certification criteria but kept the privacy and security criteria; removed model-card requirements from (b)(11) | [Reed Smith, 2026-10-30](https://www.reedsmith.com/our-insights/blogs/health-industry-washington-watch/102o6vt/onc-finalizes-hti-5-de-regulating-less-than-anticipated-and-punting-on-informati/) | M |

The demo uses made-up patients only, so none of these applies to it directly; they shape what the design must show to be credible.

## 3. Lab data for a demo

- **FHIR R4** is the standard: a `DiagnosticReport` groups `Observation`s, each with a value, unit and reference range. (From memory, standard; H.)
- **Synthea** generates made-up patients in FHIR, including lab observations; it doesn't generate everything (no appointments, messages or access logs). [Synthea wiki](https://github.com/synthetichealth/synthea/wiki/HL7-FHIR) (H)
- **Public FHIR test servers** (e.g. [HAPI R4](https://hapi.fhir.org/baseR4)) hold data anyone can change; tests use saved files instead. (H)

## 4. The four-project series: which ideas, and why

Six generic healthcare ideas were scored 1–5 on problem size, fit for a small record-system maker or virtual clinic, how well a demo works with made-up data, and how open the market is (5 = few sellers).

| Idea | Problem | Fit | Demo | Open | Total | Notes |
|---|---|---|---|---|---|---|
| Care-team message assistant | 5 | 4 | 4 | 2 | 15 | Patient messages up 153% 2020–2025; Epic drafts replies for large systems; trials showed lower mental effort, not time saved |
| Prior authorization letters | 5 | 4 | 4 | 2 | 15 | ~40 requests per physician a week (AMA 2025); free tools exist; insurer rules are private |
| Evidence pack as its own tool | 3 | 4 | 5 | 3 | 15 | ONC (g)(4) asks for a named quality system, not requirement-to-test links; better as a feature |
| Record-access audit | 3 | 4 | 3 | 4 | 14 | OCR fines for snooping ($240,000; $4.75M); sellers target hospitals; access logs must be made up |
| Pre-visit intake summary | 3 | 4 | 4 | 2 | 13 | Crowded and fast-moving; no trial evidence of time saved |
| Appointment no-show reducer | 4 | 3 | 4 | 1 | 12 | Saturated; virtual visits have fewer no-shows |

Owner's choice (2026-11-10, `docs/decisions.md`): **lab reports** and **care follow-up reminders** first, then the **care-team message assistant** and the **record-access audit**. The evidence pack is a feature of all four.

Sources for the table:

- AMA prior-authorization survey 2025: [AMA](https://www.ama-assn.org/press-center/ama-press-releases/ama-survey-prior-authorization-reform-pledge-falls-short-physicians) (H)
- Portal message growth: [telehealth.org](https://telehealth.org/news/patients-accept-ai-drafted-portal-messages-but-only-with-clinician-review/) (M-L)
- UC San Diego draft-reply study: [UCSD, 2024-04-15](https://today.ucsd.edu/story/study-reveals-ai-enhances-physician-patient-communication) (H)
- Stanford draft-reply study: [JAMA Netw Open 2024](https://jamanetwork.com/journals/jamanetworkopen/fullarticle/2816494) (H)
- Snooping settlements: [HHS, 2024-02-06](https://www.hhs.gov/about/news/2024/02/06/hhs-office-civil-rights-settles-malicious-insider-cybersecurity-investigation.html) (H) and [HealthITSecurity](https://healthitsecurity.com/news/medical-record-snooping-case-leads-to-240k-hipaa-settlement) (H)
- No-show rates: [MGMA, 2026-08-11](https://www.mgma.com/mgma-stat/about-1-in-3-medical-groups-see-higher-no-shows-in-2026-as-patients-face-higher-costs) (M) and [BMC Health Serv Res 2025](https://pmc.ncbi.nlm.nih.gov/articles/PMC12063363) (H)
- ONC (g)(4): [HHS test procedure](https://hhs.gov/guidance/sites/default/files/hhs-guidance-documents/2006093871-kq-g4tp.pdf) (H)
- Pre-visit summaries, vendors: [Abridge](https://www.abridge.com/blog/now-in-practice-august-2026) (vendor claim)

## Gaps

- Whether existing lab-report tools for small clinics check numbers in code: **not found** (not searched in depth; a short competitor scan is part of the spec).
- How long a physician spends writing a lab explanation today: **not found**.
