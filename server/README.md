# Contribution server (self-hosted)

The desktop app only shares data if (1) you set this server's URL in the app (`contribute_url`) and
(2) the user clicks **Share** in the notice dialog. Nothing is hardcoded to any host.

```bash
cd server
pip install -r requirements.txt
uvicorn app:app --host 127.0.0.1 --port 8787
```
Put it behind HTTPS (Caddy / nginx / a cloud load balancer): the app refuses plain `http://`
except for `localhost`. Then export training data and fold it into the pipeline:

```bash
python ../pipeline/01_data/merge_contributions.py    # -> pipeline/data/raw.jsonl
python ../pipeline/01_data/validate_dataset.py
```
Your obligations as the operator: publish `/privacy`, honour `DELETE /v1/contributions/{id}`,
secure the disk, and check the data protection laws that apply to your users (e.g. GDPR).
Redaction is best-effort; names written in prose are not removed - review samples before training.
