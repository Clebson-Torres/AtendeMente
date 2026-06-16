const API = "http://localhost:3001/api";

export interface TestPatient {
  id: string;
  full_name: string;
}

export interface TestAppointment {
  id: string;
  patient_id: string;
  starts_at: string;
  ends_at: string;
}

/**
 * Create a test patient via API.
 */
export async function createTestPatient(token: string): Promise<TestPatient> {
  const ts = Date.now();
  const res = await fetch(`${API}/patients`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      full_name: `E2E Patient ${ts}`,
      phone: "(11) 99999-0000",
      email: `patient-${ts}@test.com`,
    }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Failed to create test patient: ${res.status} ${body}`);
  }

  const json = await res.json();
  return { id: json.data.id, full_name: json.data.full_name };
}

/**
 * Create a test appointment via API for a given patient.
 */
export async function createTestAppointment(
  token: string,
  patientId: string,
): Promise<TestAppointment> {
  const tomorrow = new Date();
  tomorrow.setDate(tomorrow.getDate() + 1);
  tomorrow.setHours(10, 0, 0, 0);

  const endTime = new Date(tomorrow);
  endTime.setHours(11, 0, 0, 0);

  const res = await fetch(`${API}/appointments`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      patient_id: patientId,
      starts_at: tomorrow.toISOString(),
      ends_at: endTime.toISOString(),
      session_price_cents: 10000, // R$ 100,00
    }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Failed to create test appointment: ${res.status} ${body}`);
  }

  const json = await res.json();
  return json.data;
}
