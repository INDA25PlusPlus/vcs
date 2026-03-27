= Riskanalys VCS

Sannolikhet vs allvarlighet
Betygsatt av 5

- Går inte att parallellisera arbetet
    - 3, 4 = 12
    - Förhindra: Gör projektet modulärt, abstraktioner. Försöka komma över bottleneck ASAP. Unit tests för att göra det lättare att kombinera moduler.
    - Hantera: Programmera i grupp, sätta deadlines. Testa flera implementationer parallellt.
- Vi skjuter upp MVP:n till förmån för avancerade funktioner/optimering
    - 2, 5 = 10
    - Förhindra: Fokus på MVP:n. Undvik optimering i tidiga stadiet. Var inte rädd för breaking changes.
    - Hantera: Skala ned, ta bort onödiga delar.
- Någon/några i gruppen tappar intresse för projektet
    - 3, 3 = 9
    - Förhindra: Fokusera på att dela upp projektet så att det finns arbetsuppgifter åt alla.
    - Hantera: Öppna för diskussion. Byta arbetsuppgifter, tänka om kring delar av koden så att det blir mer intressant.
- Problem med immutable repo
    - 4, 2 = 8
    - Förhindra: Planera noggrant, ha i åtanke under kodningen. 
    - Hantera: Ta bort featuren (pls no). 
- Storage-abstraktionen blir för invecklad
    - 3, 2 = 6
    - Förhindra: Genomtänkt abstraktion. Abstrahera så länge det finns fördelar med abstraktionen.
    - Hantera: Hårdkoda implementationen (standard filsystem)
- Vi trackar kodbasen med VCS:en och den blir korruperad
    - 5, 1 = 5
    - Förhindra: Var bäst
    - Hantera: Det e lugnt
- Prestandan blir alldeles för dålig
    - 3, 1 = 3
    - Förhindra: Tänka ut mer övergripande implementation och design i förväg.
    - Hantera: Man kan alltid optimera mer senare.
- Vi förlorar filer till kodbasen
    - 1, 3 = 3
    - Förhindra: Backups. Pusha regelbundet till GitHub.
    - Hantera: Skriv om.
- Diffarna blir inte deterministiska
    - 1, 2 = 2
    - Förhindra: Mycket tester. Lägg till version field till commits så att det går att göra breaking changes.
    - Hantera: Skriv om diffhanteringen.
- Interfacet mellan frontend och backend blir krångligt
    - 1, 2 = 2
    - Förhindra: Tänk noga igenom abstraktioner. Planera tillsammans. Var beredd att skriva om.
    - Hantera: Skriv om ifall det behövs. Eller brösta bara.
