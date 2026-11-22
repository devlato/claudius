// The feedback page: what a person writes, sent to claudius' feedback Worker (feedback/ in the repository) as the
// studio sends it: what they wrote, where to answer them if they gave it, and that it came from the site.
const ENDPOINT = "https://claudius-feedback.devlato.workers.dev/v1/feedback";
const $ = (id) => document.getElementById(id);

async function send(event) {
  event.preventDefault();
  const text = $("text").value.trim();
  const contact = $("contact").value.trim();
  if (!text) {
    $("said").textContent = "Write something first.";
    $("text").focus();
    return;
  }
  const message = { v: 1, text, where: "site" };
  if (contact) message.contact = contact;
  $("send").disabled = true;
  $("said").textContent = "Sending…";
  try {
    const r = await fetch(ENDPOINT, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(message) });
    if (r.ok) {
      $("text").value = "";
      $("said").textContent = "Sent. Thank you.";
    } else {
      $("said").textContent = r.status === 429 ? "Not sent: too many at once. Try again in a minute." : `Not sent (${r.status}). Try again in a moment.`;
    }
  } catch {
    $("said").textContent = "Not sent: no connection. Try again in a moment.";
  } finally {
    $("send").disabled = false;
  }
}

$("feedback").addEventListener("submit", send);
