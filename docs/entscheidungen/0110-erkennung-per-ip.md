---
title: "0110: Erkennung per IP für die Webkarte"
description: Warum die Webkarte einen Spieler an der IP erkennt, wenn genau ein Spieler online sie hat, warum das nur zum Ansehen reicht, wie die IPs als HMAC vom Plugin zum Server kommen, warum X-Real-IP nur von einzelnen Proxys gilt und wie IPv6 über einen Namen nur mit IPv4 und einen Tausch geht; was verworfen ist und was es kostet.
status: gilt
date: 2026-10-20
issues: [305]
code:
  - renderer/src/cli/server/ip.rs
  - renderer/src/cli/server/api.rs
---

# 0110: Erkennung per IP für die Webkarte

## Anlass

Der User will auf der Webkarte sich selbst und die Mitspieler sehen, die er
im Mod sähe, und dafür möglichst ohne Anmeldung auskommen (#305). Den Plan
haben Plugin, Frontend, Backend und Server Manager gemeinsam geschrieben;
der Reviewer hat ihn freigegeben, die Entscheide stehen im Issue.

## Entscheidung

- **Normalfall IP:** Kommt der Browser von der IP genau eines Spielers
  online, gilt er als dieser. Teilen sich mehrere Spieler online eine IP
  oder gehört sie keinem, bleibt `/karte anmelden`. Ein gültiges Cookie
  gilt vor der IP.
- **Über die IP nur ansehen:** Mitspieler und Aktionsebenen. Klicks auf
  Knöpfe nur mit einer Sitzung aus `/karte anmelden`, sonst `403`. Jeder
  Spieler kann die Erkennung für sich abschalten, `/karte ip aus`; Vorgabe
  an. Entscheid des Users.
- **Die IPs** schreibt das Plugin als `ips`, je IP mit genau einem Spieler
  ein HMAC mit dem Geheimnis der Tokens und einem eigenen Vorsatz; der
  Server liest die Datei nur.
- **Die Adresse** ist die Gegenstelle der Verbindung. `X-Real-IP` gilt nur
  von einzelnen Adressen aus `--trusted-proxy`, nie von einem Netz;
  fehlt er dort, gilt keine Adresse. `X-Forwarded-For` gilt nie.
- **Zuschauer:** Der Server meldet erkannte Spieler mit `Web: <uuid>`; das
  Plugin schreibt die Dateien nur für sie.
- **IPv6:** Ein eigener Name nur mit IPv4 dient der Erkennung über
  `api/erkennen`, Entscheid des Users. Ohne ihn erkennt der Server nur, wer
  über IPv4 kommt.
  - Dort gibt der Server einen Tausch, 60 s und einmal, den die Karte am
    eigenen Namen gegen eine lesende Sitzung tauscht. Beide tragen das HMAC
    der IP; eine volle Sitzung entsteht so nie.
  - Die lesende Sitzung gilt nur, solange `ips` dieses HMAC auf den
    Spieler führt, und höchstens 12 h.
  - CORS nur unter `api/erkennen`, für genau den Ursprung der Karte, ohne
    Credentials; die Origin-Prüfung läuft vor jeder Arbeit.
  - Die Content-Security-Policy erlaubt der Seite dann `connect-src` zum
    zweiten Namen.

Einzelheiten: [Server](../benutzung/server.md), „Erkennung per IP“ und
„Erkennen über IPv4“, und [Plugin](../plugin.md), „Erkennung per IP“.

## Verworfene Alternativen

- **Nur Anmeldung:** Sicher, aber für jeden Blick ein Befehl im Spiel; der
  User will es ohne.
- **Volle Rechte über die IP:** Hinter einer geteilten IP könnte ein
  Fremder für den Spieler klicken. Ansehen allein hielt der User für
  tragbar, Klicken nicht.
- **Die IPs über stdin an den Server:** Die Datei liegt beim Server
  ohnehin, überlebt dessen Neustart und braucht kein neues Protokoll auf
  stdin, das heute nur das Ende anzeigt.
- **IPs im Klartext auf der Platte:** Eine Sicherung des Ordners nennte
  sie. Mit dem HMAC braucht es dafür das Geheimnis, und wer das hat, könnte
  ohnehin Sitzungen fälschen.
- **`X-Forwarded-For`:** Den ersten Wert schickt der Browser selbst, nginx
  hängt nur an.
- **Ein Netz in `--trusted-proxy`:** Ein falsch gesetztes Netz öffnete die
  Erkennung für jeden darin; einzelne Adressen schliessen das aus.
- **Lesende Sitzung für 10 min statt an `ips` gebunden:** Eine offene
  Karte müsste mitten im Betrieb sechsmal je Stunde neu tauschen, und nach
  dem Verlassen bliebe sie bis zu 10 min offen. Gebunden an `ips` endet
  sie mit der nächsten Datei; das Nachschlagen macht der Server für die
  Erkennung ohnehin.
- **Das Cookie am Namen nur mit IPv4:** Es gehörte zu einer anderen Seite
  und gälte am Namen der Karte nicht. Das Token geht deshalb durch die
  Seite, lesbar nur für den Ursprung der Karte.
- **CORS mit Credentials:** Der Tausch braucht kein Cookie; mit
  Credentials ginge Zustand des Browsers über den fremden Ursprung, ohne
  Nutzen.
- **Die Spieler als Aktionsebene:** Jede Bewegung änderte die Ebene, ihre
  Version und die Liste der Ebenen jede Sekunde, auch im Mod. Eine eigene
  Datei `sicht.json` kostet nur die Zuschauer.

## Folgen

- **Geteilte IP bleibt ein Risiko:** Wer dieselbe IP hat wie genau ein
  Spieler online, sieht dessen Mitspieler und Aktionsebenen, im selben
  WLAN wie hinter CGNAT. Das steht in der Doku und im Text an den User.
- **Dual-Stack:** Spiel über IPv4, Browser über IPv6 passt am Namen der
  Karte nie. Mit dem Namen nur mit IPv4 geht der Tausch; der Betreiber
  braucht dafür DNS und ein Zertifikat. Ohne ihn bleibt die Anmeldung.
- **Kosten nach Regel 26,** geschätzt:
  - keine Renderzeit, kein Platz beim Server; er schreibt nichts;
  - im Speicher die Tabelle `ips`, rund 100 Byte je Spieler online, und je
    Spieler die Zeit des letzten `Web:`;
  - je Anfrage unter `/api/` ohne gültiges Cookie ein HMAC und ein
    Nachschlagen; `ips` neu lesen höchstens einmal je Sekunde;
  - je Anfrage mit lesender Sitzung ein Nachschlagen in `ips`, im Cookie
    32 Byte mehr; je Tausch ein Eintrag unter den benutzten Anmeldungen
    für 60 s;
  - beim Plugin `ips` bei Join und Quit und die Ordner nur für Zuschauer.
