# Evaluation

Claude's drafts of the 30 made-up demo lab reports, each put through the checks every draft must pass before a physician can approve it (`src/checks.rs`). Replayed from `evalset/recorded/`; `cargo run -- eval --record` asks Claude again. Costs are at list price.

| Model | Pass rate | Passed | Cost per report |
|---|---|---|---|
| claude-sonnet-5-5 | 90% | 27 of 30 | US$0.0085 |
| claude-haiku-5-5 | 53% | 16 of 30 | US$0.0004 |

## claude-sonnet-5-5: drafts that failed

- **lab-report-05.json**
  - Creatinine (r3): the text puts the value on another side of the lab's range than the lab does
  - Free T4 (r15): the text negates or blurs where the value sits; it must say within, above or below plainly
- **lab-report-27.json**
  - Anti-Mullerian hormone (AMH) (r8): uses diagnosis, treatment or medication wording: "ovarian reserve"
- **lab-report-28.json**
  - Creatinine (r3): the text puts the value on another side of the lab's range than the lab does

## claude-haiku-5-5: drafts that failed

- **lab-report-01.json**
  - Free T4 (r15): the text contains a digit or a number in words
- **lab-report-02.json**
  - Hemoglobin A1c (r13): the text contains a digit or a number in words
  - Free T4 (r15): the text contains a digit or a number in words
- **lab-report-03.json**
  - Hemoglobin A1c (r13): the text contains a digit or a number in words
  - Free T4 (r15): the text contains a digit or a number in words
- **lab-report-04.json**
  - Hemoglobin A1c (r13): the text contains a digit or a number in words
  - Free T4 (r15): the text contains a digit or a number in words
- **lab-report-05.json**
  - Free T4 (r15): the text contains a digit or a number in words
- **lab-report-06.json**
  - Hemoglobin A1c (r13): the text contains a digit or a number in words
  - Free T4 (r15): the text contains a digit or a number in words
- **lab-report-07.json**
  - Hemoglobin A1c (r13): the text contains a digit or a number in words
  - Free T4 (r15): the text contains a digit or a number in words
- **lab-report-08.json**
  - Free T4 (r15): the text contains a digit or a number in words
- **lab-report-10.json**
  - Hemoglobin A1c (r13): the text contains a digit or a number in words
  - Free T4 (r15): the text contains a digit or a number in words
  - Anti-Mullerian hormone (AMH) (r23): uses diagnosis, treatment or medication wording: "ovarian reserve"
- **lab-report-14.json**
  - Hemoglobin A1c (r1): the text contains a digit or a number in words
  - Free T4 (r3): the text contains a digit or a number in words
- **lab-report-18.json**
  - Hemoglobin A1c (r1): the text contains a digit or a number in words
  - Free T4 (r3): the text contains a digit or a number in words
- **lab-report-22.json**
  - Hemoglobin A1c (r1): the text contains a digit or a number in words
  - Free T4 (r3): the text contains a digit or a number in words
- **lab-report-26.json**
  - Hemoglobin A1c (r1): the text contains a digit or a number in words
  - Free T4 (r3): the text contains a digit or a number in words
- **lab-report-30.json**
  - Hemoglobin A1c (r1): the text contains a digit or a number in words
  - Free T4 (r3): the text contains a digit or a number in words
